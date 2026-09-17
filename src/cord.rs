//! Cord: The spinal cord and reflex engine of Presence.
//! Intercepts vegetative stimuli from StemBus and executes sub-millisecond
//! involuntary reflexes (suppress, deterministic exec, or targeted escalation)
//! out of band from LLM token spend.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeReflex {
    pub id: String,
    pub trigger_stimulus: String,
    /// "suppress", "exec", "escalate"
    pub action: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default = "default_true")]
    pub fallback_to_cortex: bool,
    #[serde(default)]
    pub target_agent: Option<String>,
    #[serde(default)]
    pub expires_at: Option<u64>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub created_at: u64,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReflexOutcome {
    /// Stimulus suppressed (0 tokens, no alarm written)
    Suppressed {
        reflex_id: String,
        reason: Option<String>,
    },
    /// Autonomous command executed without LLM
    Executed {
        reflex_id: String,
        command: String,
        success: bool,
        output: String,
    },
    /// Escalated to conscious Cortex
    Escalate {
        target_agent: Option<String>,
        reason: String,
    },
}

pub struct Cord {
    pub reflexes: Vec<RuntimeReflex>,
    pub reflexes_path: PathBuf,
}

impl Cord {
    pub fn load(path: &Path) -> Self {
        let reflexes = if let Ok(s) = std::fs::read_to_string(path) {
            serde_json::from_str::<Vec<RuntimeReflex>>(&s).unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            reflexes,
            reflexes_path: path.to_path_buf(),
        }
    }

    pub fn reload(&mut self) {
        if let Ok(s) = std::fs::read_to_string(&self.reflexes_path) {
            if let Ok(r) = serde_json::from_str::<Vec<RuntimeReflex>>(&s) {
                self.reflexes = r;
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(p) = self.reflexes_path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let data = serde_json::to_string_pretty(&self.reflexes).unwrap_or_default();
        std::fs::write(&self.reflexes_path, data)
    }

    pub fn add_or_update(&mut self, reflex: RuntimeReflex) -> std::io::Result<()> {
        self.reflexes.retain(|r| r.id != reflex.id);
        self.reflexes.push(reflex);
        self.save()
    }

    pub fn remove(&mut self, reflex_id: &str) -> std::io::Result<bool> {
        let before = self.reflexes.len();
        self.reflexes.retain(|r| r.id != reflex_id);
        let changed = self.reflexes.len() < before;
        if changed {
            self.save()?;
        }
        Ok(changed)
    }

    /// Evaluate a triggered stimulus against active reflexes.
    pub fn evaluate(
        &mut self,
        stimulus_name: &str,
        default_target_agent: Option<&str>,
        default_reason: &str,
        now: u64,
        working_dir: &Path,
    ) -> ReflexOutcome {
        // Prune expired reflexes
        let mut had_expired = false;
        self.reflexes.retain(|r| {
            if let Some(exp) = r.expires_at {
                if now >= exp {
                    had_expired = true;
                    return false;
                }
            }
            true
        });
        if had_expired {
            let _ = self.save();
        }

        // Find matching reflex
        for r in &self.reflexes {
            if r.trigger_stimulus == stimulus_name {
                match r.action.as_str() {
                    "suppress" => {
                        return ReflexOutcome::Suppressed {
                            reflex_id: r.id.clone(),
                            reason: r.reason.clone(),
                        };
                    }
                    "exec" => {
                        if let Some(ref cmd) = r.command {
                            let (success, output) = run_guarded_command(cmd, working_dir);
                            if success {
                                return ReflexOutcome::Executed {
                                    reflex_id: r.id.clone(),
                                    command: cmd.clone(),
                                    success: true,
                                    output,
                                };
                            } else if r.fallback_to_cortex {
                                return ReflexOutcome::Escalate {
                                    target_agent: r.target_agent.clone().or_else(|| default_target_agent.map(String::from)),
                                    reason: format!("Reflex {} failed ({output}), escalating: {default_reason}", r.id),
                                };
                            } else {
                                return ReflexOutcome::Executed {
                                    reflex_id: r.id.clone(),
                                    command: cmd.clone(),
                                    success: false,
                                    output,
                                };
                            }
                        }
                    }
                    "escalate" => {
                        return ReflexOutcome::Escalate {
                            target_agent: r.target_agent.clone().or_else(|| default_target_agent.map(String::from)),
                            reason: r.reason.clone().unwrap_or_else(|| default_reason.to_string()),
                        };
                    }
                    _ => {}
                }
            }
        }

        // Default: escalate to target agent
        ReflexOutcome::Escalate {
            target_agent: default_target_agent.map(String::from),
            reason: default_reason.to_string(),
        }
    }
}

/// Execute a shell command bounded by ProcessGuard and timeout.
fn run_guarded_command(cmd: &str, cwd: &Path) -> (bool, String) {
    #[cfg(windows)]
    let mut child = match Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", cmd])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, format!("Failed to spawn reflex command: {e}")),
    };

    #[cfg(not(windows))]
    let mut child = match Command::new("sh")
        .args(["-c", cmd])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return (false, format!("Failed to spawn reflex command: {e}")),
    };

    let sandbox_cfg = crate::sandbox::SandboxConfig {
        max_memory_bytes: Some(256 * 1024 * 1024),
        kill_on_parent_exit: true,
        timeout_seconds: 15,
        ..Default::default()
    };
    let _guard = crate::sandbox::ProcessGuard::attach(&child, &sandbox_cfg);

    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                return (false, "Reflex command timed out (15s limit)".to_string());
            }
        }
    };

    let mut output = String::new();
    if let Some(mut r) = child.stdout.take() {
        use std::io::Read;
        let _ = r.read_to_string(&mut output);
    }
    if let Some(mut r) = child.stderr.take() {
        use std::io::Read;
        let mut err = String::new();
        let _ = r.read_to_string(&mut err);
        if !err.is_empty() {
            if !output.is_empty() {
                output.push(' ');
            }
            output.push_str(&err);
        }
    }

    (status.success(), output.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cord_reflex_suppression_and_expiry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reflexes.json");
        let mut cord = Cord::load(&path);

        // Add suppression reflex expiring at t = 2000
        let r = RuntimeReflex {
            id: "reflex:test_suppress".to_string(),
            trigger_stimulus: "disk_space_low".to_string(),
            action: "suppress".to_string(),
            command: None,
            fallback_to_cortex: true,
            target_agent: None,
            expires_at: Some(2000),
            reason: Some("Owner snoozed".to_string()),
            created_at: 1000,
        };
        cord.add_or_update(r).unwrap();

        // At t = 1500 (not expired): should suppress
        let outcome1 = cord.evaluate("disk_space_low", Some("mechanic"), "Disk full", 1500, dir.path());
        assert_eq!(
            outcome1,
            ReflexOutcome::Suppressed {
                reflex_id: "reflex:test_suppress".to_string(),
                reason: Some("Owner snoozed".to_string()),
            }
        );

        // At t = 2050 (expired): should prune and escalate
        let outcome2 = cord.evaluate("disk_space_low", Some("mechanic"), "Disk full", 2050, dir.path());
        assert_eq!(
            outcome2,
            ReflexOutcome::Escalate {
                target_agent: Some("mechanic".to_string()),
                reason: "Disk full".to_string(),
            }
        );
        assert_eq!(cord.reflexes.len(), 0);
    }

    #[test]
    fn test_cord_reflex_exec_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reflexes.json");
        let mut cord = Cord::load(&path);

        let r = RuntimeReflex {
            id: "reflex:test_echo".to_string(),
            trigger_stimulus: "workspace_bloat".to_string(),
            action: "exec".to_string(),
            command: Some("echo reflex_clean_ok".to_string()),
            fallback_to_cortex: true,
            target_agent: Some("mechanic".to_string()),
            expires_at: None,
            reason: Some("Auto clean".to_string()),
            created_at: 1000,
        };
        cord.add_or_update(r).unwrap();

        let outcome = cord.evaluate("workspace_bloat", Some("mechanic"), "Bloat detected", 1500, dir.path());
        match outcome {
            ReflexOutcome::Executed { reflex_id, success, output, .. } => {
                assert_eq!(reflex_id, "reflex:test_echo");
                assert!(success);
                assert!(output.contains("reflex_clean_ok"), "output={output}");
            }
            other => panic!("Unexpected outcome: {other:?}"),
        }
    }
}