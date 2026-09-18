//! presence configuration — single store (owner directive 2026-09-15, modernized 2026-09-17).
//! All runtime values live in `.config/presence.yaml`;
//! binaries and the lib read it through here.
//!
//! Precedence:
//!   - If `override_from_env: true`: env vars (OPENAI_*, PRESENCE_*) > yaml > compiled defaults.
//!   - If `override_from_env: false`: yaml is source of truth (falls back to env keys only if yaml keys are empty).
//!
//! Autonomous workspace:
//!   - `paths.workspace` / `$PRESENCE_WORKSPACE` replaces the legacy `haven` dependency.
//!   - Hierarchical fallback stacks: Models -> Endpoints -> API Keys.
//!   - DeepSeek alignment with `prompt_cache_key` in `capabilities`.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyInstallPolicy {
    Auto,
    Ignore,
    Warn,
}

impl Default for DependencyInstallPolicy {
    fn default() -> Self {
        DependencyInstallPolicy::Warn
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default)]
pub struct DependenciesCfg {
    pub install_policy: DependencyInstallPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub override_from_env: bool,
    #[serde(default)]
    pub models: Vec<ModelEntryCfg>,
    /// Legacy fallback for configs with `model: ModelCfg`
    #[serde(default)]
    pub model: Option<ModelCfg>,
    pub paths: PathsCfg,
    pub voice: VoiceCfg,
    pub limits: LimitsCfg,
    pub heartbeat: HeartbeatCfg,
    pub senses: SensesCfg,
    pub lang: String,
    #[serde(default)]
    pub dependencies: DependenciesCfg,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelCfg {
    pub base_url: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelEntryCfg {
    pub name: String,
    pub context_limit: u64,
    pub max_output_tokens: u64,
    pub capabilities: ModelCapabilitiesCfg,
    pub endpoints: Vec<EndpointCfg>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ModelCapabilitiesCfg {
    pub tools: bool,
    pub parallel_tool_calls: bool,
    pub prompt_cache_key: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct EndpointCfg {
    pub base_url: String,
    #[serde(default)]
    pub api_keys: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct SensesCfg {
    pub time: bool,
    pub hearing: bool,
    pub vision: bool,
    pub proprioception: bool,
    pub windows: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct PathsCfg {
    /// Relative to USERPROFILE, or absolute.
    pub desk: PathBuf,
    /// Workspace root (autonomous presence workspace).
    pub workspace: Option<PathBuf>,
    /// Bridge script. Defaults to <workspace>/bridge/proxy.mjs.
    pub bridge: Option<PathBuf>,
    /// Memory dir. Defaults to <workspace>/memory.
    pub memory_dir: Option<PathBuf>,
    /// Tools dir. Defaults to <workspace>/tools.
    pub tools_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct VoiceCfg {
    pub tts_voice: String,
    pub vosk_model: PathBuf,
    /// Empty = auto-detect (Headset preferred).
    pub mic: String,
    pub sample_rate: u32,
    /// Was PRESENCE_EAR_ON=1.
    pub ear_on: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct LimitsCfg {
    pub tool_output: usize,
    pub tool_output_model: usize,
    pub run_command_timeout_secs: u64,
    pub listen_default_secs: u64,
    pub listen_max_secs: u64,
    pub chat_max_tokens: u64,
    pub chat_tool_rounds: usize,
    pub ctx_limit: u64,
    pub pin_chars: usize,
    pub transcript_tail: usize,
    pub other_topic_tail: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct HeartbeatCfg {
    pub interval_secs: u64,
    /// 0 = run forever.
    pub max_wakes: u64,
    /// Initiative act rate cap = attempt debounce.
    pub cap_secs: u64,
    pub max_alarms: usize,
    pub max_attempts: u32,
}

pub fn default_models() -> Vec<ModelEntryCfg> {
    vec![
        ModelEntryCfg {
            name: "deepseek-v4-flash".into(),
            context_limit: 1_048_576,
            max_output_tokens: 8192,
            capabilities: ModelCapabilitiesCfg {
                tools: true,
                parallel_tool_calls: true,
                prompt_cache_key: true,
            },
            endpoints: vec![EndpointCfg {
                base_url: "https://agentrouter.org/v1".into(),
                api_keys: Vec::new(),
            }],
        },
        ModelEntryCfg {
            name: "glm-5.3".into(),
            context_limit: 128_000,
            max_output_tokens: 4096,
            capabilities: ModelCapabilitiesCfg {
                tools: true,
                parallel_tool_calls: false,
                prompt_cache_key: false,
            },
            endpoints: vec![EndpointCfg {
                base_url: "https://agentrouter.org/v1".into(),
                api_keys: Vec::new(),
            }],
        },
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            override_from_env: false,
            models: default_models(),
            model: None,
            paths: PathsCfg::default(),
            voice: VoiceCfg::default(),
            limits: LimitsCfg::default(),
            heartbeat: HeartbeatCfg::default(),
            senses: SensesCfg::default(),
            lang: "ru".into(),
            dependencies: DependenciesCfg::default(),
        }
    }
}

impl Default for ModelCfg {
    fn default() -> Self {
        Self {
            base_url: "https://agentrouter.org/v1".into(),
            name: "glm-5.3".into(),
        }
    }
}

impl Default for ModelEntryCfg {
    fn default() -> Self {
        Self {
            name: "deepseek-v4-flash".into(),
            context_limit: 1_048_576,
            max_output_tokens: 8192,
            capabilities: ModelCapabilitiesCfg::default(),
            endpoints: vec![EndpointCfg::default()],
        }
    }
}

impl Default for ModelCapabilitiesCfg {
    fn default() -> Self {
        Self {
            tools: true,
            parallel_tool_calls: true,
            prompt_cache_key: true,
        }
    }
}

impl Default for EndpointCfg {
    fn default() -> Self {
        Self {
            base_url: "https://agentrouter.org/v1".into(),
            api_keys: Vec::new(),
        }
    }
}

impl Default for SensesCfg {
    fn default() -> Self {
        Self {
            time: true,
            hearing: true,
            vision: false,
            proprioception: true,
            windows: true,
        }
    }
}

impl Default for PathsCfg {
    fn default() -> Self {
        Self {
            desk: PathBuf::from("desk"),
            workspace: None,
            bridge: None,
            memory_dir: None,
            tools_dir: None,
        }
    }
}

impl Default for VoiceCfg {
    fn default() -> Self {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        Self {
            tts_voice: "ru-RU-DmitryNeural".into(),
            vosk_model: PathBuf::from(&home).join(r".vosk\vosk-model-small-ru-0.22"),
            mic: String::new(),
            sample_rate: 16000,
            ear_on: false,
        }
    }
}

impl Default for LimitsCfg {
    fn default() -> Self {
        Self {
            tool_output: 8000,
            tool_output_model: 4000,
            run_command_timeout_secs: 30,
            listen_default_secs: 6,
            listen_max_secs: 30,
            chat_max_tokens: 2000,
            chat_tool_rounds: 8,
            ctx_limit: 0, // 0 = model-table lookup (budgeter)
            pin_chars: 4000,
            transcript_tail: 16,
            other_topic_tail: 3,
        }
    }
}

impl Default for HeartbeatCfg {
    fn default() -> Self {
        Self {
            interval_secs: 5,
            max_wakes: 0,
            cap_secs: 1200,
            max_alarms: 20,
            max_attempts: 3,
        }
    }
}

/// Absolute as-is, ~/ expanded, relative joined against USERPROFILE.
fn resolve_home(p: PathBuf) -> PathBuf {
    let p_str = p.to_string_lossy();
    if p_str.starts_with("~/") || p_str.starts_with("~\\") {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        return Path::new(&home).join(&p_str[2..]);
    }
    if p.is_absolute() {
        p
    } else {
        let home = std::env::var("USERPROFILE").unwrap_or_default();
        Path::new(&home).join(p)
    }
}

fn get_env_var(keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Ok(val) = std::env::var(k) {
            let val = val.trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

/// Active model and endpoint parameters ready for execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveTarget {
    pub model_name: String,
    pub base_url: String,
    pub api_key: String,
    pub context_limit: u64,
    pub max_output_tokens: u64,
    pub capabilities: ModelCapabilitiesCfg,
}

/// Tiered failover outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailoverTier {
    Tier1KeyRotation {
        endpoint: String,
        from_key_index: usize,
        to_key_index: usize,
    },
    Tier2EndpointRotation {
        from_endpoint: String,
        to_endpoint: String,
    },
    Tier3ModelRotation {
        from_model: String,
        to_model: String,
    },
}

/// Resilient fallback state machine across Models -> Endpoints -> Keys.
#[derive(Debug, Clone)]
pub struct FailoverStack {
    pub models: Vec<ModelEntryCfg>,
    pub override_from_env: bool,
    pub active_model_idx: usize,
    pub active_endpoint_idx: usize,
    pub active_key_idx: usize,
}

impl FailoverStack {
    pub fn new(cfg: &Config) -> Self {
        let mut models = cfg.effective_models();
        let env_key = get_env_var(&["OPENAI_API_KEY", "PRESENCE_API_KEY", "PRESENCE_API_KEY"]);
        let env_base_url = get_env_var(&["OPENAI_BASE_URL", "PRESENCE_BASE_URL", "PRESENCE_BASE_URL"]);
        let env_model = get_env_var(&["OPENAI_MODEL", "PRESENCE_MODEL", "PRESENCE_MODEL"]);

        if cfg.override_from_env {
            if let Some(m_name) = env_model {
                if let Some(first_m) = models.first_mut() {
                    first_m.name = m_name;
                }
            }
            if let Some(b_url) = env_base_url {
                if let Some(first_m) = models.first_mut() {
                    if let Some(first_ep) = first_m.endpoints.first_mut() {
                        first_ep.base_url = b_url;
                    }
                }
            }
            if let Some(k) = env_key {
                if let Some(first_m) = models.first_mut() {
                    if let Some(first_ep) = first_m.endpoints.first_mut() {
                        first_ep.api_keys.insert(0, k);
                    }
                }
            }
        } else {
            // override_from_env is false: YAML is source of truth.
            // If endpoints have no keys specified, populate from env as fallback for secrets.
            if let Some(k) = env_key {
                for m in &mut models {
                    for ep in &mut m.endpoints {
                        if ep.api_keys.is_empty() {
                            ep.api_keys.push(k.clone());
                        }
                    }
                }
            }
        }

        Self {
            models,
            override_from_env: cfg.override_from_env,
            active_model_idx: 0,
            active_endpoint_idx: 0,
            active_key_idx: 0,
        }
    }

    pub fn current_target(&self) -> Option<ActiveTarget> {
        let model = self.models.get(self.active_model_idx)?;
        let endpoint = model.endpoints.get(self.active_endpoint_idx)?;
        let api_key = endpoint
            .api_keys
            .get(self.active_key_idx)
            .cloned()
            .unwrap_or_default();

        Some(ActiveTarget {
            model_name: model.name.clone(),
            base_url: endpoint.base_url.clone(),
            api_key,
            context_limit: model.context_limit,
            max_output_tokens: model.max_output_tokens,
            capabilities: model.capabilities.clone(),
        })
    }

    /// Advance failover based on error message/status code:
    /// - Tier 1 (401 / 429): rotate to next key on current endpoint
    /// - Tier 2 (5xx / timeout / exhausted keys): rotate to next endpoint
    /// - Tier 3 (exhausted endpoints): rotate to next model
    pub fn on_error(&mut self, err_msg: &str) -> Result<FailoverTier, String> {
        let is_auth_or_ratelimit = err_msg.contains("401")
            || err_msg.contains("429")
            || err_msg.to_ascii_lowercase().contains("unauthorized")
            || err_msg.to_ascii_lowercase().contains("rate limit");

        let current_model = match self.models.get(self.active_model_idx) {
            Some(m) => m,
            None => return Err("No active model available".into()),
        };
        let current_endpoint = match current_model.endpoints.get(self.active_endpoint_idx) {
            Some(ep) => ep,
            None => return Err("No active endpoint available".into()),
        };

        // Tier 1: Try key rotation if auth/ratelimit and more keys remain
        if is_auth_or_ratelimit && self.active_key_idx + 1 < current_endpoint.api_keys.len() {
            let from_key = self.active_key_idx;
            self.active_key_idx += 1;
            return Ok(FailoverTier::Tier1KeyRotation {
                endpoint: current_endpoint.base_url.clone(),
                from_key_index: from_key,
                to_key_index: self.active_key_idx,
            });
        }

        // Tier 2: Try endpoint rotation if keys exhausted or server error/timeout
        if self.active_endpoint_idx + 1 < current_model.endpoints.len() {
            let from_ep = current_endpoint.base_url.clone();
            self.active_endpoint_idx += 1;
            self.active_key_idx = 0;
            let to_ep = self.models[self.active_model_idx].endpoints[self.active_endpoint_idx]
                .base_url
                .clone();
            return Ok(FailoverTier::Tier2EndpointRotation {
                from_endpoint: from_ep,
                to_endpoint: to_ep,
            });
        }

        // Tier 3: Try model rotation if all endpoints on current model exhausted
        if self.active_model_idx + 1 < self.models.len() {
            let from_m = current_model.name.clone();
            self.active_model_idx += 1;
            self.active_endpoint_idx = 0;
            self.active_key_idx = 0;
            let to_m = self.models[self.active_model_idx].name.clone();
            return Ok(FailoverTier::Tier3ModelRotation {
                from_model: from_m,
                to_model: to_m,
            });
        }

        Err("All fallback models, endpoints, and API keys exhausted".into())
    }

    /// Record failover event in friction ledger if memory directory is accessible.
    pub fn record_failover_friction(
        &self,
        memory_dir: &Path,
        session_id: &str,
        tier: &FailoverTier,
        reason: &str,
    ) {
        let detail = match tier {
            FailoverTier::Tier1KeyRotation {
                endpoint,
                from_key_index,
                to_key_index,
            } => {
                format!(
                    "Tier1 key rotation on {endpoint}: key {from_key_index} -> {to_key_index} ({reason})"
                )
            }
            FailoverTier::Tier2EndpointRotation {
                from_endpoint,
                to_endpoint,
            } => {
                format!("Tier2 endpoint rotation: {from_endpoint} -> {to_endpoint} ({reason})")
            }
            FailoverTier::Tier3ModelRotation {
                from_model,
                to_model,
            } => {
                format!("Tier3 model rotation: {from_model} -> {to_model} ({reason})")
            }
        };
        crate::friction::record(
            memory_dir,
            session_id,
            crate::friction::FrictionKind::Failover,
            &detail,
        );
    }
}

impl Config {
    /// Find and load the config. Never panics: a missing or broken
    /// yaml falls back to compiled defaults (with a note to stderr).
    pub fn load() -> Config {
        let path = Self::find();
        match path {
            Some(p) => match std::fs::read_to_string(&p) {
                Ok(text) => match serde_yaml::from_str::<Config>(&text) {
                    Ok(c) => {
                        eprintln!("presence: config loaded from {}", p.display());
                        c
                    }
                    Err(e) => {
                        eprintln!("presence: config parse error in {} — defaults: {e}", p.display());
                        Config::default()
                    }
                },
                Err(e) => {
                    eprintln!("presence: config read error {} — defaults: {e}", p.display());
                    Config::default()
                }
            },
            None => {
                eprintln!("presence: no .config/presence.yaml found — compiled defaults");
                Config::default()
            }
        }
    }

    /// Process-wide cached instance (loads once).
    pub fn cached() -> &'static Config {
        static C: std::sync::OnceLock<Config> = std::sync::OnceLock::new();
        C.get_or_init(Config::load)
    }

    fn find() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("PRESENCE_CONFIG").or_else(|_| std::env::var("PRESENCE_CONFIG")) {
            let p = PathBuf::from(p);
            if p.is_file() {
                return Some(p);
            }
            return None;
        }
        if let Some(w) = get_env_var(&["PRESENCE_WORKSPACE", "PRESENCE_WORKSPACE", "PRESENCE_WORKSPACE"]) {
            for sub in [".config/presence.yaml", ".config/presence.yaml", "presence/.config/presence.yaml", "presence/.config/presence.yaml"] {
                let cand = PathBuf::from(&w).join(sub);
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            let p = PathBuf::from(home).join(".presence/.config/presence.yaml");
            if p.is_file() {
                return Some(p);
            }
        }
        let mut dir = std::env::current_dir().ok()?;
        loop {
            for sub in [".config/presence.yaml", ".config/presence.yaml", "presence/.config/presence.yaml", "presence/.config/presence.yaml"] {
                let cand = dir.join(sub);
                if cand.is_file() {
                    return Some(cand);
                }
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    /// Return models list, guaranteeing non-empty. If YAML models is empty,
    /// falls back to legacy `model` field or `default_models()`.
    pub fn effective_models(&self) -> Vec<ModelEntryCfg> {
        if !self.models.is_empty() {
            return self.models.clone();
        }
        if let Some(m) = &self.model {
            return vec![ModelEntryCfg {
                name: m.name.clone(),
                context_limit: 128_000,
                max_output_tokens: 4096,
                capabilities: ModelCapabilitiesCfg {
                    tools: true,
                    parallel_tool_calls: false,
                    prompt_cache_key: false,
                },
                endpoints: vec![EndpointCfg {
                    base_url: m.base_url.clone(),
                    api_keys: Vec::new(),
                }],
            }];
        }
        default_models()
    }

    /// Instantiate a fresh failover stack based on this config.
    pub fn failover_stack(&self) -> FailoverStack {
        FailoverStack::new(self)
    }

    /// Return active target parameters (model, base_url, api_key, limits, capabilities).
    pub fn active_target(&self) -> ActiveTarget {
        self.failover_stack()
            .current_target()
            .unwrap_or_else(|| ActiveTarget {
                model_name: self.model_name(),
                base_url: self.base_url(),
                api_key: self.api_key(),
                context_limit: 128_000,
                max_output_tokens: 4096,
                capabilities: ModelCapabilitiesCfg::default(),
            })
    }

    /// Workspace root (yaml side only — no env, testable):
    /// paths.workspace or paths.workspace resolved against USERPROFILE when relative.
    pub fn workspace_from_cfg(&self) -> Option<PathBuf> {
        self.paths
            .workspace
            .as_ref()
            .or(self.paths.workspace.as_ref())
            .map(|p| resolve_home(p.clone()))
    }

    /// Autonomous workspace root:
    /// Env PRESENCE_WORKSPACE > PRESENCE_WORKSPACE > yaml paths.workspace > paths.workspace.
    pub fn workspace_path(&self) -> Option<PathBuf> {
        if let Some(w) = get_env_var(&["PRESENCE_WORKSPACE", "PRESENCE_WORKSPACE", "PRESENCE_WORKSPACE"]) {
            return Some(resolve_home(PathBuf::from(w)));
        }
        self.workspace_from_cfg()
    }





    /// Bridge script: env PRESENCE_BRIDGE > yaml > derived from workspace.
    pub fn bridge_path(&self) -> Option<PathBuf> {
        if let Some(p) = get_env_var(&["PRESENCE_BRIDGE", "PRESENCE_BRIDGE"]) {
            return Some(PathBuf::from(p));
        }
        if let Some(p) = self.paths.bridge.clone() {
            return Some(resolve_home(p));
        }
        self.workspace_path().map(|w| {
            if w.join("bridge/proxy.mjs").exists() {
                w.join("bridge/proxy.mjs")
            } else if w.join("tools/presence/bridge/proxy.mjs").exists() {
                w.join("tools/presence/bridge/proxy.mjs")
            } else {
                w.join("bridge/proxy.mjs")
            }
        })
    }

    /// Memory dir: env PRESENCE_MEMORY > yaml > derived from workspace.
    pub fn memory_dir_path(&self) -> Option<PathBuf> {
        if let Some(p) = get_env_var(&["PRESENCE_MEMORY", "PRESENCE_MEMORY"]) {
            return Some(PathBuf::from(p));
        }
        if let Some(p) = self.paths.memory_dir.clone() {
            return Some(resolve_home(p));
        }
        self.workspace_path().map(|w| {
            if w.join("memory").exists() {
                w.join("memory")
            } else if w.join("tools/presence/memory").exists() {
                w.join("tools/presence/memory")
            } else {
                w.join("memory")
            }
        })
    }

    /// Tools dir: env PRESENCE_TOOLS > yaml > derived from workspace.
    pub fn tools_dir_path(&self) -> Option<PathBuf> {
        if let Some(p) = get_env_var(&["PRESENCE_TOOLS", "PRESENCE_TOOLS"]) {
            return Some(PathBuf::from(p));
        }
        if let Some(p) = self.paths.tools_dir.clone() {
            return Some(resolve_home(p));
        }
        self.workspace_path().map(|w| w.join("tools"))
    }

    /// Absolute desk path (env override > yaml; relative resolved against USERPROFILE).
    pub fn desk_path(&self) -> PathBuf {
        if let Some(p) = get_env_var(&["PRESENCE_DESK", "PRESENCE_DESK"]) {
            return PathBuf::from(p);
        }
        let p = &self.paths.desk;
        resolve_home(p.clone())
    }

    /// Model name: handles override_from_env toggle, OPENAI_MODEL > PRESENCE_MODEL > yaml.
    pub fn model_name(&self) -> String {
        if self.override_from_env {
            if let Some(m) = get_env_var(&["OPENAI_MODEL", "PRESENCE_MODEL", "PRESENCE_MODEL"]) {
                return m;
            }
        }
        self.effective_models()
            .first()
            .map(|m| m.name.clone())
            .unwrap_or_else(|| "deepseek-v4-flash".into())
    }

    /// Base URL: handles override_from_env toggle, OPENAI_BASE_URL > PRESENCE_BASE_URL > yaml.
    pub fn base_url(&self) -> String {
        if self.override_from_env {
            if let Some(u) = get_env_var(&["OPENAI_BASE_URL", "PRESENCE_BASE_URL", "PRESENCE_BASE_URL"]) {
                return u;
            }
        }
        self.effective_models()
            .first()
            .and_then(|m| m.endpoints.first().map(|ep| ep.base_url.clone()))
            .unwrap_or_else(|| "https://agentrouter.org/v1".into())
    }

    /// Active API key: handles override_from_env, OPENAI_API_KEY > PRESENCE_API_KEY > yaml.
    pub fn api_key(&self) -> String {
        let env_k = get_env_var(&["OPENAI_API_KEY", "PRESENCE_API_KEY", "PRESENCE_API_KEY"]);
        if self.override_from_env {
            if let Some(k) = env_k {
                return k;
            }
        }
        let yaml_key = self
            .effective_models()
            .first()
            .and_then(|m| m.endpoints.first())
            .and_then(|ep| ep.api_keys.first().cloned())
            .filter(|k| !k.is_empty());

        yaml_key.or(env_k).unwrap_or_default()
    }

    }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal() {
        let c: Config = serde_yaml::from_str("").unwrap();
        assert_eq!(c.model_name(), "deepseek-v4-flash");
        assert_eq!(c.limits.tool_output, 8000);
        assert!(c.workspace_from_cfg().is_none());
        assert!(c.senses.time);
        assert!(c.senses.hearing);
        assert!(!c.senses.vision);
        assert!(c.senses.proprioception);
    }

    #[test]
    fn parse_override() {
        let y = "models:\n  - name: gpt-x\n    endpoints:\n      - base_url: https://api.openai.com/v1\nlimits:\n  tool_output: 123\n";
        let c: Config = serde_yaml::from_str(y).unwrap();
        assert_eq!(c.model_name(), "gpt-x");
        assert_eq!(c.base_url(), "https://api.openai.com/v1");
        assert_eq!(c.limits.tool_output, 123);
    }

    #[test]
    fn parse_legacy_model_field() {
        let y = "model:\n  name: glm-5.3\n  base_url: https://agentrouter.org/v1\n";
        let c: Config = serde_yaml::from_str(y).unwrap();
        assert_eq!(c.model_name(), "glm-5.3");
        assert_eq!(c.base_url(), "https://agentrouter.org/v1");
    }

    #[test]
    fn desk_resolves_against_home() {
        let c: Config = serde_yaml::from_str("paths:\n  desk: desk").unwrap();
        let d = c.desk_path();
        assert!(d.is_absolute());
        assert!(d.ends_with("desk"));
    }

    #[test]
    fn workspace_yaml_derives_bridge_and_memory() {
        let c: Config = serde_yaml::from_str("paths:\n  workspace: C:/tmp/hv").unwrap();
        assert_eq!(c.workspace_from_cfg(), Some(PathBuf::from("C:/tmp/hv")));
        let b = c.paths.bridge.clone();
        assert!(b.is_none());
        assert_eq!(
            c.workspace_from_cfg().map(|h| h.join("bridge/proxy.mjs")),
            Some(PathBuf::from("C:/tmp/hv/bridge/proxy.mjs"))
        );
    }

    #[test]
    fn workspace_yaml_preferred_and_derives_paths() {
        let c: Config = serde_yaml::from_str("paths:\n  workspace: C:/presence_ws").unwrap();
        assert_eq!(c.workspace_from_cfg(), Some(PathBuf::from("C:/presence_ws")));
        assert_eq!(
            c.bridge_path(),
            Some(PathBuf::from("C:/presence_ws/bridge/proxy.mjs"))
        );
        assert_eq!(
            c.memory_dir_path(),
            Some(PathBuf::from("C:/presence_ws/memory"))
        );
        assert_eq!(
            c.tools_dir_path(),
            Some(PathBuf::from("C:/presence_ws/tools"))
        );
    }

    #[test]
    fn workspace_yaml_relative_resolves_against_home() {
        let c: Config = serde_yaml::from_str("paths:\n  workspace: myworkspace").unwrap();
        let h = c.workspace_from_cfg().unwrap();
        assert!(h.is_absolute());
        assert!(h.ends_with("myworkspace"));
    }

    #[test]
    fn explicit_bridge_and_memory_win_over_derivation() {
        let y = "paths:\n  workspace: C:/tmp/ws\n  bridge: C:/b/proxy.mjs\n  memory_dir: C:/mem\n";
        let c: Config = serde_yaml::from_str(y).unwrap();
        assert_eq!(c.paths.bridge, Some(PathBuf::from("C:/b/proxy.mjs")));
        assert_eq!(c.paths.memory_dir, Some(PathBuf::from("C:/mem")));
    }

    #[test]
    fn prompt_cache_key_in_capabilities() {
        let y = r#"
models:
  - name: deepseek-v4-flash
    context_limit: 1048576
    max_output_tokens: 8192
    capabilities:
      tools: true
      parallel_tool_calls: true
      prompt_cache_key: true
    endpoints:
      - base_url: https://agentrouter.org/v1
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        let m = &c.models[0];
        assert_eq!(m.name, "deepseek-v4-flash");
        assert_eq!(m.context_limit, 1_048_576);
        assert_eq!(m.max_output_tokens, 8192);
        assert!(m.capabilities.prompt_cache_key);
        assert!(m.capabilities.parallel_tool_calls);
    }

    #[test]
    fn failover_tier1_key_rotation() {
        let y = r#"
models:
  - name: test-model
    endpoints:
      - base_url: https://ep1.test/v1
        api_keys: ["key1", "key2", "key3"]
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        let mut stack = c.failover_stack();

        let t1 = stack.current_target().unwrap();
        assert_eq!(t1.api_key, "key1");

        let res = stack.on_error("HTTP 401 Unauthorized").unwrap();
        assert_eq!(
            res,
            FailoverTier::Tier1KeyRotation {
                endpoint: "https://ep1.test/v1".into(),
                from_key_index: 0,
                to_key_index: 1,
            }
        );
        assert_eq!(stack.current_target().unwrap().api_key, "key2");

        let res2 = stack.on_error("rate limit 429").unwrap();
        assert_eq!(
            res2,
            FailoverTier::Tier1KeyRotation {
                endpoint: "https://ep1.test/v1".into(),
                from_key_index: 1,
                to_key_index: 2,
            }
        );
        assert_eq!(stack.current_target().unwrap().api_key, "key3");
    }

    #[test]
    fn failover_tier2_endpoint_rotation() {
        let y = r#"
models:
  - name: test-model
    endpoints:
      - base_url: https://ep1.test/v1
        api_keys: ["ep1_k1"]
      - base_url: https://ep2.test/v1
        api_keys: ["ep2_k1"]
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        let mut stack = c.failover_stack();

        // 500 error triggers endpoint rotation
        let res = stack.on_error("502 Bad Gateway").unwrap();
        assert_eq!(
            res,
            FailoverTier::Tier2EndpointRotation {
                from_endpoint: "https://ep1.test/v1".into(),
                to_endpoint: "https://ep2.test/v1".into(),
            }
        );
        let t = stack.current_target().unwrap();
        assert_eq!(t.base_url, "https://ep2.test/v1");
        assert_eq!(t.api_key, "ep2_k1");
    }

    #[test]
    fn failover_tier3_model_rotation() {
        let y = r#"
models:
  - name: primary-model
    endpoints:
      - base_url: https://ep1.test/v1
        api_keys: ["k1"]
  - name: backup-model
    endpoints:
      - base_url: https://ep2.test/v1
        api_keys: ["k2"]
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        let mut stack = c.failover_stack();

        // Single endpoint on primary fails -> rotates to backup model
        let res = stack.on_error("500 Internal Server Error").unwrap();
        assert_eq!(
            res,
            FailoverTier::Tier3ModelRotation {
                from_model: "primary-model".into(),
                to_model: "backup-model".into(),
            }
        );
        let t = stack.current_target().unwrap();
        assert_eq!(t.model_name, "backup-model");
        assert_eq!(t.base_url, "https://ep2.test/v1");
        assert_eq!(t.api_key, "k2");
    }

    #[test]
    fn failover_exhaustion() {
        let y = r#"
models:
  - name: only-model
    endpoints:
      - base_url: https://ep1.test/v1
        api_keys: ["k1"]
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        let mut stack = c.failover_stack();

        let err = stack.on_error("503 Service Unavailable").unwrap_err();
        assert!(err.contains("exhausted"));
    }

    #[test]
    fn override_from_env_false_protects_yaml() {
        let y = r#"
override_from_env: false
models:
  - name: yaml-model
    endpoints:
      - base_url: https://yaml-ep.test/v1
        api_keys: ["yaml_key"]
"#;
        let c: Config = serde_yaml::from_str(y).unwrap();
        assert!(!c.override_from_env);
        assert_eq!(c.model_name(), "yaml-model");
        assert_eq!(c.base_url(), "https://yaml-ep.test/v1");
        assert_eq!(c.api_key(), "yaml_key");
    }
    #[test]
    fn parse_dependencies_install_policy_default_warn() {
        let c = Config::default();
        assert_eq!(c.dependencies.install_policy, DependencyInstallPolicy::Warn);
    }

    #[test]
    fn parse_dependencies_install_policy_auto_and_ignore() {
        let y_auto = "dependencies:\n  install_policy: auto\n";
        let c_auto: Config = serde_yaml::from_str(y_auto).unwrap();
        assert_eq!(c_auto.dependencies.install_policy, DependencyInstallPolicy::Auto);

        let y_ignore = "dependencies:\n  install_policy: ignore\n";
        let c_ignore: Config = serde_yaml::from_str(y_ignore).unwrap();
        assert_eq!(c_ignore.dependencies.install_policy, DependencyInstallPolicy::Ignore);
    }
}

