//! StemBus: Autonomous vegetative stimulus scheduler and event dispatcher.
//! Gathers stimuli declarations from installed organs (ToolManifest.stimuli)
//! and executes periodic telemetry checks out of band from LLM token budget.

use crate::tools::{discover_dynamic_tools, execute_dynamic_tool, DiscoveredTool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveStimulus {
    pub organ_name: String,
    pub stimulus_name: String,
    pub description: Option<String>,
    pub cadence_secs: u64,
    pub action: String,
    #[serde(default)]
    pub target_agent: Option<String>,
    #[serde(skip)]
    pub tool: Option<DiscoveredTool>,
    pub last_polled: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StimulusEvent {
    pub organ_name: String,
    pub stimulus_name: String,
    pub triggered: bool,
    pub action: String,
    #[serde(default)]
    pub target_agent: Option<String>,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StimulusOverride {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub snooze_until: Option<u64>,
    #[serde(default)]
    pub cadence_secs: Option<u64>,
    #[serde(default)]
    pub reason: Option<String>,
}

impl Default for StimulusOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            snooze_until: None,
            cadence_secs: None,
            reason: None,
        }
    }
}

fn default_true() -> bool {
    true
}

pub fn load_stimuli_overrides(path: &std::path::Path) -> std::collections::HashMap<String, StimulusOverride> {
    if let Ok(data) = std::fs::read_to_string(path) {
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    }
}

pub fn save_stimuli_overrides(
    path: &std::path::Path,
    overrides: &std::collections::HashMap<String, StimulusOverride>,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let data = serde_json::to_string_pretty(overrides).unwrap_or_default();
    std::fs::write(path, data)
}

pub struct StemBus {
    pub stimuli: Vec<ActiveStimulus>,
}

impl StemBus {
    /// Discover all stimuli from workspace and system installed organs.
    pub fn discover() -> Self {
        let tools = discover_dynamic_tools();
        Self::from_tools(&tools)
    }

    pub fn from_tools(tools: &[DiscoveredTool]) -> Self {
        let mut stimuli = Vec::new();
        for t in tools {
            for s in &t.manifest.stimuli {
                stimuli.push(ActiveStimulus {
                    organ_name: t.manifest.name.clone(),
                    stimulus_name: s.name.clone(),
                    description: s.description.clone(),
                    cadence_secs: s.cadence_secs.unwrap_or(30),
                    action: s.action.clone().unwrap_or_else(|| "alert".to_string()),
                    target_agent: s.target_agent.clone(),
                    tool: Some(t.clone()),
                    last_polled: 0,
                });
            }
        }
        Self { stimuli }
    }


    /// Poll all active stimuli whose cadence interval has elapsed, respecting overrides and snoozes.
    pub fn poll_due(
        &mut self,
        now: u64,
        overrides: &std::collections::HashMap<String, StimulusOverride>,
    ) -> Vec<StimulusEvent> {
        let mut events = Vec::new();
        for stim in &mut self.stimuli {
            if let Some(ov) = overrides.get(&stim.stimulus_name) {
                if !ov.enabled {
                    continue;
                }
                if let Some(snooze) = ov.snooze_until {
                    if now < snooze {
                        continue;
                    }
                }
            }

            let cadence = overrides
                .get(&stim.stimulus_name)
                .and_then(|ov| ov.cadence_secs)
                .unwrap_or(stim.cadence_secs);

            if now.saturating_sub(stim.last_polled) >= cadence {
                stim.last_polled = now;
                if let Some(event) = Self::poll_one(stim) {
                    events.push(event);
                }
            }
        }
        events
    }

    pub fn poll_one(stim: &ActiveStimulus) -> Option<StimulusEvent> {
        let tool = stim.tool.as_ref()?;
        let args = json!({
            "stimulus": stim.stimulus_name
        });
        let raw = execute_dynamic_tool(tool, &args);
        let parsed: Value = serde_json::from_str(&raw).ok()?;
        let triggered = parsed.get("triggered").and_then(|v| v.as_bool()).unwrap_or(false);

        Some(StimulusEvent {
            organ_name: stim.organ_name.clone(),
            stimulus_name: stim.stimulus_name.clone(),
            triggered,
            action: stim.action.clone(),
            target_agent: stim.target_agent.clone(),
            data: parsed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::tools::{OrganStimulusDef, ToolManifest, ToolPermissions};

    #[test]
    fn test_stembus_creation_and_cadence() {
        let manifest = ToolManifest {
            name: "test_organ".to_string(),
            version: Some("1.0.0".into()),
            description: "A test sensory organ".into(),
            instructions: None,
            entrypoint: Some("organ-test.exe".into()),
            r#type: Some("cli".into()),
            parameters: json!({}),
            senses: vec![],
            tools: vec![],
            permissions: ToolPermissions::default(),
            commands: vec![],
            stimuli: vec![
                OrganStimulusDef {
                    name: "heartbeat_sensor".into(),
                    description: Some("Sensor test".into()),
                    cadence_secs: Some(15),
                    action: Some("modulate_pulse".into()),
target_agent: Some("arche".into()),
                },
                OrganStimulusDef {
                    name: "temperature_spike".into(),
                    description: Some("Thermal monitor".into()),
                    cadence_secs: Some(60),
                    action: Some("alert".into()),
target_agent: Some("mechanic".into()),
                },
            ],
            reflexes: vec![],
        };

        let discovered = vec![DiscoveredTool {
            manifest,
            dir: PathBuf::from("."),
        }];

        let mut bus = StemBus::from_tools(&discovered);
        assert_eq!(bus.stimuli.len(), 2);
        assert_eq!(bus.stimuli[0].cadence_secs, 15);
        assert_eq!(bus.stimuli[1].cadence_secs, 60);

        // Simulation timer
        let t0 = 1000u64;
        assert!(t0.saturating_sub(bus.stimuli[0].last_polled) >= 15);
        assert!(t0.saturating_sub(bus.stimuli[1].last_polled) >= 60);

        bus.stimuli[0].last_polled = t0;
        bus.stimuli[1].last_polled = t0;

        let t1 = t0 + 10;
        assert!(t1.saturating_sub(bus.stimuli[0].last_polled) < 15);
        assert!(t1.saturating_sub(bus.stimuli[1].last_polled) < 60);

        let t2 = t0 + 20;
        assert!(t2.saturating_sub(bus.stimuli[0].last_polled) >= 15);
        assert!(t2.saturating_sub(bus.stimuli[1].last_polled) < 60);
    }
}