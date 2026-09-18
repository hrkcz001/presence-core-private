//! Tool set v0 (PLAN §6). OpenAI tool definitions + executor.
//! write_file is desk-scoped (step 15): paths resolve under the desk
//! root, no escape. run_command is bounded: 30s timeout, output capped.
//! Reads are unrestricted. Brain writes are runtime-owned, never here.

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::Config;

fn cfg() -> &'static Config {
    Config::cached()
}

pub struct ToolCtx {
    pub desk: PathBuf,
    /// Pinned excerpts shared with the daemon (context.pin/evict).
    pub pinned: std::sync::Mutex<std::collections::HashMap<String, String>>,
    /// Speaking window (unix ts start, end) - echo avoidance: the ear
    /// skips utterances inside it.
    /// Dynamic sensory organ overrides (attunement / sensory gating).
    pub senses_mask: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, bool>>>,
    pub persona: Option<String>,
}

impl Default for ToolCtx {
    fn default() -> Self {
        Self {
            desk: PathBuf::from("."),
            pinned: Default::default(),
            senses_mask: Default::default(),
            persona: None,
        }
    }
}

pub fn current_persona(ctx: &ToolCtx) -> String {
    if let Some(ref p) = ctx.persona {
        return p.clone();
    }
    let mem_cand = ctx.desk.join("memory/agent.active");
    if let Ok(s) = std::fs::read_to_string(mem_cand) {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Some(mem) = cfg().memory_dir_path() {
        if let Ok(s) = std::fs::read_to_string(mem.join("agent.active")) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "arche".to_string()
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrganToolDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OrganSenseDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub poll_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct OrganStimulusDef {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cadence_secs: Option<u64>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub target_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct OrganReflexDef {
    pub on: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OrganCompatibility {
    #[serde(default)]
    pub presence: Option<String>,
    #[serde(default)]
    pub api_version: Option<u32>,
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
    #[serde(default)]
    pub features: Option<Vec<String>>,
}


/// Dependency on another Presence organ.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OrganDep {
    pub name: String,
    #[serde(default)]
    pub optional: bool,
}

/// Dependency on a host system binary (managed via Scoop or Nix/Guix).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SystemDep {
    pub binary: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub scoop: Option<String>,
    #[serde(default)]
    pub nix: Option<String>,
    #[serde(default)]
    pub guix: Option<String>,
    #[serde(default)]
    pub disables_tools: Vec<String>,
    #[serde(default)]
    pub disables_features: Vec<String>,
}

impl SystemDep {
    pub fn target_package(&self) -> Option<&str> {
        #[cfg(windows)]
        {
            self.scoop.as_deref().or(Some(&self.binary))
        }
        #[cfg(not(windows))]
        {
            if std::env::var("GUIX_ENVIRONMENT").is_ok() || which_cmd("guix") {
                self.guix.as_deref().or(self.nix.as_deref()).or(Some(&self.binary))
            } else {
                self.nix.as_deref().or(self.guix.as_deref()).or(Some(&self.binary))
            }
        }
    }

    pub fn install_hint(&self) -> String {
        let pkg = self.target_package().unwrap_or(&self.binary);
        #[cfg(windows)]
        {
            format!("packager_install(package: \"{pkg}\") [host fallback: scoop install {pkg}]")
        }
        #[cfg(not(windows))]
        {
            if std::env::var("GUIX_ENVIRONMENT").is_ok() || which_cmd("guix") {
                format!("packager_install(package: \"{pkg}\") [host fallback: guix install {pkg}]")
            } else {
                format!("packager_install(package: \"{pkg}\") [host fallback: nix-env -iA nixpkgs.{pkg}]")
            }
        }
    }
}

/// Specification of organ dependencies in organ.yaml.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OrganDependencies {
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub organs: Vec<OrganDep>,
    #[serde(default)]
    pub system: Vec<SystemDep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyReport {
    pub is_runnable: bool,
    pub runtime_ok: bool,
    pub missing_runtime: Option<String>,
    pub missing_required_organs: Vec<String>,
    pub missing_optional_organs: Vec<String>,
    pub missing_required_system: Vec<SystemDep>,
    pub missing_optional_system: Vec<SystemDep>,
    pub disabled_tools: Vec<String>,
    pub disabled_features: Vec<String>,
}

impl OrganDependencies {
    pub fn evaluate(&self, mounted_organs: &[String]) -> DependencyReport {
        let mut report = DependencyReport {
            is_runnable: true,
            runtime_ok: true,
            missing_runtime: None,
            missing_required_organs: Vec::new(),
            missing_optional_organs: Vec::new(),
            missing_required_system: Vec::new(),
            missing_optional_system: Vec::new(),
            disabled_tools: Vec::new(),
            disabled_features: Vec::new(),
        };

        if let Some(rt) = &self.runtime {
            if !which_cmd(rt) {
                report.runtime_ok = false;
                report.missing_runtime = Some(rt.clone());
                report.is_runnable = false;
            }
        }

        for od in &self.organs {
            let present = mounted_organs.iter().any(|m| m.eq_ignore_ascii_case(&od.name));
            if !present {
                if od.optional {
                    report.missing_optional_organs.push(od.name.clone());
                } else {
                    report.missing_required_organs.push(od.name.clone());
                    report.is_runnable = false;
                }
            }
        }

        for sd in &self.system {
            let present = which_cmd(&sd.binary);
            if !present {
                if sd.optional {
                    report.disabled_tools.extend(sd.disables_tools.clone());
                    report.disabled_features.extend(sd.disables_features.clone());
                    report.missing_optional_system.push(sd.clone());
                } else {
                    report.missing_required_system.push(sd.clone());
                    report.is_runnable = false;
                }
            }
        }

        report
    }
}

impl OrganCompatibility {
    pub fn validate(&self, host_presence_version: &str) -> Result<(), String> {
        if let Some(req_str) = &self.presence {
            let req = VersionReq::parse(req_str)
                .map_err(|e| format!("Invalid SemVer presence constraint '{req_str}': {e}"))?;
            let host_ver = Version::parse(host_presence_version)
                .map_err(|e| format!("Invalid host presence version '{host_presence_version}': {e}"))?;
            if !req.matches(&host_ver) {
                return Err(format!(
                    "Presence version requirement '{req_str}' not satisfied by host '{host_presence_version}'"
                ));
            }
        }
        if let Some(platforms) = &self.platforms {
            let os = std::env::consts::OS;
            if !platforms.iter().any(|p| p.eq_ignore_ascii_case(os)) {
                return Err(format!(
                    "Platform '{os}' is not supported; required: {:?}",
                    platforms
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolManifest {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub manual: Option<String>,
    pub description: String,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub parameters: Value,
    #[serde(default)]
    pub senses: Vec<OrganSenseDef>,
    #[serde(default)]
    pub tools: Vec<OrganToolDef>,
    #[serde(default)]
    pub permissions: ToolPermissions,
    #[serde(default, alias = "slash_commands")]
    pub commands: Vec<Value>,
    #[serde(default)]
    pub stimuli: Vec<OrganStimulusDef>,
    #[serde(default)]
    pub reflexes: Vec<OrganReflexDef>,
    #[serde(default)]
    pub compatibility: Option<OrganCompatibility>,
    #[serde(default)]
    pub dependencies: Option<OrganDependencies>,
    #[serde(default)]
    pub core: Option<bool>,
    #[serde(default)]
    pub ui: Option<Value>,
}

impl ToolManifest {
    pub fn is_compatible(&self) -> Result<(), String> {
        if let Some(compat) = &self.compatibility {
            compat.validate(env!("CARGO_PKG_VERSION"))
        } else {
            Ok(())
        }
    }

    pub fn slash_commands(&self) -> Vec<String> {
        let mut cmds = vec![self.name.to_lowercase()];
        for c in &self.commands {
            if let Some(s) = c.as_str() {
                let trimmed = s.trim_start_matches('/').to_lowercase();
                if !trimmed.is_empty() && !cmds.contains(&trimmed) {
                    cmds.push(trimmed);
                }
            } else if let Some(n) = c.get("name").and_then(|v| v.as_str()) {
                let trimmed = n.trim_start_matches('/').to_lowercase();
                if !trimmed.is_empty() && !cmds.contains(&trimmed) {
                    cmds.push(trimmed);
                }
            }
        }
        cmds
    }

    pub fn command_descriptors(&self) -> Vec<(String, String)> {
        let mut descs = Vec::new();
        for c in &self.commands {
            if let Some(s) = c.as_str() {
                let trimmed = s.trim_start_matches('/').to_lowercase();
                if !trimmed.is_empty() && !descs.iter().any(|(n, _): &(String, String)| n == &trimmed) {
                    descs.push((trimmed, format!("Execute {} command", self.name)));
                }
            } else if let Some(n) = c.get("name").and_then(|v| v.as_str()) {
                let trimmed = n.trim_start_matches('/').to_lowercase();
                let desc = c.get("description")
                    .and_then(|v| v.as_str())
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| format!("Execute {} command", self.name));
                if !trimmed.is_empty() && !descs.iter().any(|(n, _): &(String, String)| n == &trimmed) {
                    descs.push((trimmed, desc));
                }
            }
        }
        if descs.is_empty() {
            descs.push((self.name.to_lowercase(), self.description.clone()));
        }
        descs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolPermissions {
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub filesystem_read: bool,
    #[serde(default)]
    pub filesystem_write: bool,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredTool {
    pub manifest: ToolManifest,
    pub dir: PathBuf,
}

pub fn detect_language(path: &str) -> Option<&'static str> {
    let p = Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext.to_lowercase().as_str() {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "js" | "mjs" | "cjs" => Some("javascript"),
        "ts" => Some("typescript"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "md" | "markdown" => Some("markdown"),
        "sh" | "bash" => Some("bash"),
        "ps1" => Some("powershell"),
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" => Some("cpp"),
        "html" | "htm" => Some("html"),
        "css" => Some("css"),
        _ => None,
    }
}

pub fn format_markdown_block(content: &str, lang: Option<&str>) -> String {
    let tag = lang.unwrap_or("");
    format!("```{tag}
{content}
```")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from)
}


pub fn build_agent_path() -> std::ffi::OsString {
    let mut paths = Vec::new();
    for dt in discover_dynamic_tools() {
        let bin = dt.dir.join("bin");
        if bin.is_dir() {
            paths.push(bin);
        }
        paths.push(dt.dir.clone());
    }
    if let Some(sys_path) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&sys_path) {
            paths.push(p);
        }
    }
    std::env::join_paths(paths).unwrap_or_default()
}

pub fn discover_dynamic_tools() -> Vec<DiscoveredTool> {
    let mut search_dirs = Vec::new();
    // 1. Workspace organs & tools
    search_dirs.push(PathBuf::from("workspace/organs"));
    search_dirs.push(PathBuf::from("organs"));
    search_dirs.push(PathBuf::from("../../workspace/organs"));
    search_dirs.push(PathBuf::from("workspace/tools"));
    search_dirs.push(PathBuf::from("tools"));
    search_dirs.push(PathBuf::from("../../workspace/tools"));

    if let Ok(presence_workspace) = std::env::var("PRESENCE_WORKSPACE") {
        let root = PathBuf::from(presence_workspace);
        search_dirs.push(root.join("workspace/tools"));
        search_dirs.push(root.join("tools"));
    }
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = PathBuf::from(manifest_dir);
        search_dirs.push(root.join("../../workspace/tools"));
        search_dirs.push(root.join("../workspace/tools"));
    }

    // 2. User workspace and config directories
    if let Some(home) = dirs_home() {
        search_dirs.push(home.join(".presence/tools"));
        search_dirs.push(home.join(".config/presence/tools"));
        search_dirs.push(home.join(".config/presence/tools"));
    }

    // 3. Scoop Apps (Windows): %USERPROFILE%\scoop\apps\*\current
    let scoop_root = std::env::var("SCOOP")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs_home().map(|h| h.join("scoop")));
    if let Some(scoop) = scoop_root {
        let apps = scoop.join("apps");
        if apps.is_dir() {
            if let Ok(app_entries) = std::fs::read_dir(&apps) {
                for app in app_entries.flatten() {
                    let current = app.path().join("current");
                    if current.is_dir() {
                        search_dirs.push(current.clone());
                        search_dirs.push(current.join("tools"));
                    }
                }
            }
        }
    }

    // 4. User and Config Organs
    if let Some(home) = dirs_home() {
        search_dirs.push(home.join(".presence/organs"));
        search_dirs.push(home.join(".config/presence/organs"));
    }

    // 5. Nix Profiles (Linux / macOS / NixOS)
    if let Some(home) = dirs_home() {
        search_dirs.push(home.join(".nix-profile/share/presence/organs"));
        search_dirs.push(home.join(".nix-profile/share/presence/tools"));
    }
    search_dirs.push(PathBuf::from("/nix/var/nix/profiles/default/share/presence/organs"));
    search_dirs.push(PathBuf::from("/nix/var/nix/profiles/default/share/presence/tools"));
    search_dirs.push(PathBuf::from("/run/current-system/sw/share/presence/organs"));

    // 6. GNU Guix Profiles
    if let Some(home) = dirs_home() {
        search_dirs.push(home.join(".guix-profile/share/presence/organs"));
        search_dirs.push(home.join(".guix-profile/share/presence/tools"));
    }
    search_dirs.push(PathBuf::from("/run/current-system/profile/share/presence/organs"));
    search_dirs.push(PathBuf::from("/run/current-system/profile/share/presence/tools"));

    // 7. PRESENCE_ORGANS_PATH override
    if let Ok(env_paths) = std::env::var("PRESENCE_ORGANS_PATH") {
        for p in std::env::split_paths(&env_paths) {
            search_dirs.push(p);
        }
    }

    discover_manifests_in_dirs(&search_dirs)
}

pub fn discover_manifests_in_dirs(dirs: &[PathBuf]) -> Vec<DiscoveredTool> {
    let mut discovered = Vec::new();
    let mut seen = std::collections::HashSet::new();

    let check_file = |manifest_path: &Path, dir: &Path, discovered: &mut Vec<DiscoveredTool>, seen: &mut std::collections::HashSet<String>| {
        if manifest_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(manifest_path) {
                let clean_content = content.trim_start_matches('\u{feff}');
                if let Ok(manifest) = serde_yaml::from_str::<ToolManifest>(clean_content) {
                    if let Err(reason) = manifest.is_compatible() {
                        eprintln!("presence: organ '{}' skipped (incompatible): {reason}", manifest.name);
                        return;
                    }
                    if let Some(deps) = &manifest.dependencies {
                        let seen_vec: Vec<String> = seen.iter().cloned().collect();
                        let mut report = deps.evaluate(&seen_vec);

                        let policy = cfg().dependencies.install_policy;
                        match policy {
                            crate::config::DependencyInstallPolicy::Auto => {
                                if !report.is_runnable || !report.missing_required_system.is_empty() {
                                    for dep in &report.missing_required_system {
                                        let pkg_name = dep.target_package().unwrap_or(&dep.binary);
                                        eprintln!("presence: auto-installing missing dependency '{pkg_name}' for organ '{}'...", manifest.name);
                                        let _ = execute_package_manager("install", Some(pkg_name));
                                    }
                                    report = deps.evaluate(&seen_vec);
                                }
                                if !report.is_runnable {
                                    eprintln!("presence: organ '{}' skipped even after auto-install: system: {:?}, runtime: {:?}", manifest.name, report.missing_required_system, report.missing_runtime);
                                    return;
                                }
                            }
                            crate::config::DependencyInstallPolicy::Ignore => {
                                if !report.is_runnable {
                                    return;
                                }
                            }
                            crate::config::DependencyInstallPolicy::Warn => {
                                if !report.is_runnable {
                                    eprintln!("presence: ALARM - organ '{}' skipped (unmet dependencies): system: {:?}, runtime: {:?}", manifest.name, report.missing_required_system, report.missing_runtime);
                                    return;
                                } else if !report.disabled_tools.is_empty() || !report.disabled_features.is_empty() {
                                    eprintln!("presence: ALARM - organ '{}' mounted with degraded features: disabled tools: {:?}, disabled features: {:?}", manifest.name, report.disabled_tools, report.disabled_features);
                                }
                            }
                        }
                    }
                    if !seen.contains(&manifest.name) {
                        seen.insert(manifest.name.clone());
                        discovered.push(DiscoveredTool {
                            manifest,
                            dir: dir.to_path_buf(),
                        });
                    }
                }
            }
        }
    };

    for base in dirs {
        if !base.is_dir() {
            continue;
        }
        // Check if base itself is a tool directory (e.g. scoop/apps/<app>/current)
        check_file(&base.join("organ.yaml"), base, &mut discovered, &mut seen);
        check_file(&base.join("presence-organ.yaml"), base, &mut discovered, &mut seen);
        check_file(&base.join("tool.yaml"), base, &mut discovered, &mut seen);
        check_file(&base.join("presence-tool.yaml"), base, &mut discovered, &mut seen);
        check_file(&base.join("presence-tool.yaml"), base, &mut discovered, &mut seen);

        // Check subdirectories
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                check_file(&path.join("organ.yaml"), &path, &mut discovered, &mut seen);
                check_file(&path.join("presence-organ.yaml"), &path, &mut discovered, &mut seen);
                check_file(&path.join("tool.yaml"), &path, &mut discovered, &mut seen);
                check_file(&path.join("presence-tool.yaml"), &path, &mut discovered, &mut seen);
                check_file(&path.join("presence-tool.yaml"), &path, &mut discovered, &mut seen);
            }
        }
    }
    discovered
}

pub fn defs() -> Vec<Value> {
    let mut list = vec![
        json!({
            "type": "function",
            "function": {
                "name": "manage_package",
                "description": "Install, update, or list dynamic tools and dependencies via system package manager (Scoop on Windows, Nix on Linux/macOS).",
                "parameters": {"type": "object", "properties": {
                    "action": {"type": "string", "enum": ["install", "update", "list"], "description": "Action to perform"},
                    "package": {"type": "string", "description": "Package name to install or update (optional for list)"}
                }, "required": ["action"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "switch_agent",
                "description": "Switch the active agent persona (e.g. 'presence', 'organcrafter', 'mechanic'). Updates memory/agent.active and loads the persona card immediately.",
                "parameters": {"type": "object", "properties": {
                    "agent_name": {"type": "string", "description": "Name of the persona to activate"}
                }, "required": ["agent_name"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "tune_senses",
                "description": "Dynamically enable or disable sensory organs (vision, hearing, windows, time, proprioception) for the current task/session.",
                "parameters": {"type": "object", "properties": {
                    "sense": {"type": "string", "enum": ["vision", "hearing", "windows", "time", "proprioception", "all"], "description": "Sensory organ to tune or 'all'"},
                    "enabled": {"type": "boolean", "description": "true to enable/open the sense, false to gate/mute it"}
                }, "required": ["sense", "enabled"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "tune_stimuli",
                "description": "Manual override to inspect, snooze, disable, or enable vegetative stimuli. NOTE: For recurring patterns or automated handling, prefer registering runtime Cord reflexes so reactions execute deterministically at 0 tokens without waking Cortex.",
                "parameters": {"type": "object", "properties": {
                    "stimulus": {"type": "string", "description": "Target stimulus name or 'all'"},
                    "action": {"type": "string", "enum": ["snooze", "enable", "disable", "status"], "description": "Action to perform"},
                    "duration_seconds": {"type": "integer", "description": "Duration in seconds to snooze the stimulus (e.g. 14400 for 4 hours)"},
                    "reason": {"type": "string", "description": "Optional human-readable explanation for why it was snoozed or disabled"}
                }, "required": ["action"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "manage_reflexes",
                "description": "Register, list, or remove runtime Cord reflexes. Cord reflexes execute deterministic sub-millisecond reactions (suppression, automated shell cleanup) upon vegetative stimuli trigger without spending LLM tokens.",
                "parameters": {"type": "object", "properties": {
                    "action": {"type": "string", "enum": ["register", "list", "remove"], "description": "Action to perform"},
                    "id": {"type": "string", "description": "Optional unique ID for the reflex (e.g. reflex:disk_space:auto_clean). Required for remove."},
                    "stimulus": {"type": "string", "description": "Vegetative stimulus to trigger on (e.g. disk_space_low, workspace_bloat, git_dirty_drift)"},
                    "reflex_action": {"type": "string", "enum": ["suppress", "exec", "escalate"], "description": "Reflex behavior: 'suppress' (mutes wake), 'exec' (runs deterministic command), 'escalate' (wakes specific target agent)"},
                    "command": {"type": "string", "description": "Shell command to run when reflex_action is 'exec'"},
                    "duration_seconds": {"type": "integer", "description": "Optional lifetime in seconds before this reflex expires (e.g. 14400 for 4 hours)"},
                    "fallback_to_cortex": {"type": "boolean", "description": "If true (default), failures in 'exec' will escalate to Cortex"},
                    "target_agent": {"type": "string", "description": "Agent persona to target if escalated"},
                    "reason": {"type": "string", "description": "Human-readable intent or reason for this reflex"}
                }, "required": ["action"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a text file (any path on this machine).",
                "parameters": {"type": "object", "properties": {
                    "path": {"type": "string", "description": "Absolute or cwd-relative path"}
                }, "required": ["path"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write a text file. Relative paths resolve against workspace root; absolute paths are permitted. Auto-creates parent directories.",
                "parameters": {"type": "object", "properties": {
                    "path": {"type": "string", "description": "File path under the desk"},
                    "content": {"type": "string", "description": "Full file content"}
                }, "required": ["path", "content"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_files",
                "description": "List a directory's files and subdirectories.",
                "parameters": {"type": "object", "properties": {
                    "path": {"type": "string", "description": "Directory path (default: desk)"}
                }, "required": []}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "context_pin",
                "description": "Pin a file excerpt into the context (kept at high priority, never digested away). Reads the file and stores up to ~4000 chars.",
                "parameters": {"type": "object", "properties": {
                    "path": {"type": "string", "description": "File to pin"}
                }, "required": ["path"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "context_evict",
                "description": "Remove a pinned file from the context.",
                "parameters": {"type": "object", "properties": {
                    "path": {"type": "string", "description": "Pinned path to evict"}
                }, "required": ["path"]}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "context_status",
                "description": "Show what is pinned in the context.",
                "parameters": {"type": "object", "properties": {}, "required": []}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_command",
                "description": "Run a shell command via sh -c, 30s timeout, output truncated to ~8KB. Use for builds, git, and other machine tasks.",
                "parameters": {"type": "object", "properties": {
                    "command": {"type": "string", "description": "The command line"}
                }, "required": ["command"]}
            }
        }),
    ];

    for dt in discover_dynamic_tools() {
        let disabled_tools = if let Some(deps) = &dt.manifest.dependencies {
            let report = deps.evaluate(&[]);
            report.disabled_tools
        } else {
            Vec::new()
        };

        if !dt.manifest.tools.is_empty() {
            for t in &dt.manifest.tools {
                if disabled_tools.contains(&t.name) {
                    continue;
                }
                if !list.iter().any(|existing| {
                    existing.pointer("/function/name").and_then(Value::as_str) == Some(&t.name)
                }) {
                    list.push(json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    }));
                }
            }
        } else if !list.iter().any(|existing| {
            existing.pointer("/function/name").and_then(Value::as_str) == Some(&dt.manifest.name)
        }) {
            if !disabled_tools.contains(&dt.manifest.name) {
                list.push(json!({
                    "type": "function",
                    "function": {
                        "name": dt.manifest.name,
                        "description": dt.manifest.description,
                        "parameters": dt.manifest.parameters,
                    }
                }));
            }
        }
    }

    list
}

/// Lexical normalization (resolve `.`/`..` without touching the disk).
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Resolve a write target.
/// Relative paths resolve against workspace root (ctx.desk); absolute paths are permitted.
pub fn resolve_write_path(ctx: &ToolCtx, path: &str) -> Result<PathBuf, String> {
    if path.trim().is_empty() {
        return Err("path cannot be empty".into());
    }
    let p = Path::new(path);
    let target = if p.is_absolute() {
        p.to_path_buf()
    } else {
        ctx.desk.join(p)
    };
    Ok(normalize(&target))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... [truncated]", &s[..end])
}

pub fn detect_package_manager() -> &'static str {
    if let Ok(mgr) = std::env::var("PRESENCE_PKG_MANAGER").or_else(|_| std::env::var("PRESENCE_PKG_MANAGER")) {
        if mgr.eq_ignore_ascii_case("nix") {
            return "nix";
        }
        if mgr.eq_ignore_ascii_case("scoop") {
            return "scoop";
        }
    }
    if std::env::var("NIX_PROFILES").is_ok() || Path::new("/nix/store").exists() {
        return "nix";
    }
    "scoop"
}

pub fn execute_package_manager(action: &str, package: Option<&str>) -> String {
    let mgr = detect_package_manager();
    match mgr {
        "nix" => {
            let mut cmd = Command::new("nix");
            match action {
                "install" => {
                    let pkg = package.unwrap_or("");
                    if pkg.is_empty() {
                        return "Error: package name required for install".to_string();
                    }
                    cmd.args(["profile", "install", pkg]);
                }
                "update" => {
                    cmd.args(["profile", "upgrade"]);
                    if let Some(pkg) = package.filter(|p| !p.is_empty()) {
                        cmd.arg(pkg);
                    } else {
                        cmd.arg(".*");
                    }
                }
                "list" => {
                    cmd.args(["profile", "list"]);
                }
                _ => return format!("Error: unknown action '{action}'"),
            }
            run_cmd_output(cmd)
        }
        _ => {
            let mut cmd = if cfg!(windows) {
                let mut c = Command::new("cmd");
                c.arg("/C").arg("scoop");
                c
            } else {
                Command::new("scoop")
            };
            match action {
                "install" => {
                    let pkg = package.unwrap_or("");
                    if pkg.is_empty() {
                        return "Error: package name required for install".to_string();
                    }
                    cmd.args(["install", pkg]);
                }
                "update" => {
                    cmd.arg("update");
                    if let Some(pkg) = package.filter(|p| !p.is_empty()) {
                        cmd.arg(pkg);
                    }
                }
                "list" => {
                    cmd.arg("list");
                }
                _ => return format!("Error: unknown action '{action}'"),
            }
            run_cmd_output(cmd)
        }
    }
}

fn run_cmd_output(mut cmd: Command) -> String {
    match cmd.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            if out.status.success() {
                if stdout.trim().is_empty() && !stderr.trim().is_empty() {
                    stderr.to_string()
                } else {
                    stdout.to_string()
                }
            } else {
                format!("Failed (exit code {:?}):\nstdout:\n{stdout}\nstderr:\n{stderr}", out.status.code())
            }
        }
        Err(e) => format!("Failed to spawn package manager command: {e}"),
    }
}

fn which_cmd(cmd: &str) -> bool {
    if let Some(paths) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&paths) {
            let direct = p.join(cmd);
            if direct.is_file() {
                return true;
            }
            #[cfg(windows)]
            {
                let with_exe = p.join(format!("{cmd}.exe"));
                let with_cmd = p.join(format!("{cmd}.cmd"));
                if with_exe.is_file() || with_cmd.is_file() {
                    return true;
                }
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let shim_dir = std::path::PathBuf::from(userprofile).join("scoop").join("shims");
            let cmd_clean = cmd.trim_end_matches(".exe");
            let shim = shim_dir.join(format!("{cmd_clean}.exe"));
            if shim.is_file() {
                return true;
            }
        }
    }
    false
}

pub fn execute_dynamic_tool(tool: &DiscoveredTool, args: &Value) -> String {
    let manifest = &tool.manifest;
    let entry_name = manifest.entrypoint.as_deref().unwrap_or(manifest.name.as_str());
    let entry_path = if Path::new(entry_name).is_absolute() {
        PathBuf::from(entry_name)
    } else {
        let mut candidates = vec![
            tool.dir.join(entry_name),
            tool.dir.join(format!("{entry_name}.exe")),
            tool.dir.join("bin").join(entry_name),
            tool.dir.join("bin").join(format!("{entry_name}.exe")),
            tool.dir.join("target/release").join(entry_name),
            tool.dir.join("target/release").join(format!("{entry_name}.exe")),
        ];
        if let Ok(organs_env) = std::env::var("PRESENCE_ORGANS_PATH") {
            candidates.push(PathBuf::from(&organs_env).join(&manifest.name).join(entry_name));
            candidates.push(PathBuf::from(&organs_env).join(&manifest.name).join(format!("{entry_name}.exe")));
        }
        if let Some(home) = dirs_home() {
            candidates.push(home.join(".presence/organs").join(&manifest.name).join(entry_name));
            candidates.push(home.join(".presence/organs").join(&manifest.name).join(format!("{entry_name}.exe")));
            candidates.push(home.join("scoop/shims").join(format!("{entry_name}.exe")));
            candidates.push(home.join("scoop/shims").join(entry_name));
        }
        let presence_workspace = std::env::var("PRESENCE_WORKSPACE")
            .or_else(|_| std::env::var("PRESENCE_WORKSPACE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        candidates.push(presence_workspace.join("tools").join(&manifest.name).join(entry_name));
        if let Ok(cur_exe) = std::env::current_exe() {
            if let Some(dir) = cur_exe.parent() {
                candidates.push(dir.join(entry_name));
                candidates.push(dir.join(format!("{entry_name}.exe")));
                if let Some(p2) = dir.parent() {
                    candidates.push(p2.join(entry_name));
                    candidates.push(p2.join(format!("{entry_name}.exe")));
                }
            }
        }
        candidates.push(PathBuf::from("C:/Users/hrkcz001/Dev/presence/target/release").join(entry_name));
        candidates.push(PathBuf::from("C:/Users/hrkcz001/Dev/presence/target/release").join(format!("{entry_name}.exe")));
        candidates.push(PathBuf::from("C:/Users/hrkcz001/Dev/presence-organs/target/release").join(format!("organ-{entry_name}.exe")));
        candidates.push(PathBuf::from("C:/Users/hrkcz001/Dev/presence-organs/target/release").join(format!("{entry_name}.exe")));

        candidates.into_iter().find(|p| p.exists()).unwrap_or_else(|| tool.dir.join(entry_name))
    };

    let timeout_secs = manifest.permissions.timeout_seconds.unwrap_or(30);

    let is_python = manifest.r#type.as_deref() == Some("python")
        || entry_path.extension().and_then(|e| e.to_str()) == Some("py");
    let is_ts = manifest.r#type.as_deref() == Some("typescript")
        || entry_path.extension().and_then(|e| e.to_str()) == Some("ts");
    let is_js = manifest.r#type.as_deref() == Some("javascript")
        || entry_path.extension().and_then(|e| e.to_str()) == Some("js");

    let mut cmd = if is_python {
        let mut c = Command::new("python");
        c.env("PATH", build_agent_path());
        c.arg(&entry_path);
        c
    } else if is_ts || is_js {
        if !which_cmd("bun") {
            return format!("tool '{}' execution error: 'bun' runtime is required for TypeScript organs. Install via scoop install bun or bun.sh", manifest.name);
        }
        let mut c = Command::new("bun");
        c.env("PATH", build_agent_path());
        c.arg("run");
        c.arg(&entry_path);
        c
    } else {
        let mut c = Command::new(&entry_path);
        c.env("PATH", build_agent_path());
        c
    };

    if manifest.name == "webfetch" {
        let action = args.get("action").and_then(Value::as_str).unwrap_or("search");
        cmd.arg(action);
        if action == "search" {
            if let Some(n) = args.get("num_results").and_then(Value::as_u64) {
                cmd.arg("-n").arg(n.to_string());
            }
        }
        if let Some(target) = args.get("target").and_then(Value::as_str) {
            cmd.arg(target);
        }
    } else if is_python {
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                cmd.arg(format!("--{k}"));
                match v {
                    Value::String(s) => {
                        cmd.arg(s);
                    }
                    Value::Number(n) => {
                        cmd.arg(n.to_string());
                    }
                    Value::Bool(b) => {
                        cmd.arg(b.to_string());
                    }
                    other => {
                        cmd.arg(serde_json::to_string(other).unwrap_or_default());
                    }
                }
            }
        }
    } else if let Some(obj) = args.as_object() {
        for (k, v) in obj {
            cmd.arg(format!("--{k}"));
            if let Some(s) = v.as_str() {
                cmd.arg(s);
            } else {
                cmd.arg(v.to_string());
            }
        }
    }

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return format!("tool '{}' execution error: {e}", manifest.name),
    };

    let sandbox_cfg = crate::sandbox::SandboxConfig {
        max_memory_bytes: Some(512 * 1024 * 1024),
        kill_on_parent_exit: true,
        timeout_seconds: timeout_secs,
        ..Default::default()
    };
    let _guard = crate::sandbox::ProcessGuard::attach(&child, &sandbox_cfg);

    let mut out_r = child.stdout.take().expect("stdout");
    let mut err_r = child.stderr.take().expect("stderr");
    let t_out = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out_r.read_to_end(&mut b);
        b
    });
    let t_err = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err_r.read_to_end(&mut b);
        b
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return format!("tool '{}' error: timeout after {}s, killed", manifest.name, timeout_secs);
            }
            Err(e) => return format!("tool '{}' error: {e}", manifest.name),
        }
    };

    let out_b = t_out.join().unwrap_or_default();
    let err_b = t_err.join().unwrap_or_default();
    let out = String::from_utf8_lossy(&out_b);
    let err = String::from_utf8_lossy(&err_b);

    let mut res = out.to_string();
    if !err.trim().is_empty() {
        if !res.is_empty() {
            res.push('\n');
        }
        res.push_str("[stderr]
");
        res.push_str(&err);
    }
    if !status.success() {
        res = format!("exit: {status}
{res}");
    }
    truncate(&res, cfg().limits.tool_output)
}


fn find_organ_shell_binary(desk: &Path) -> Option<PathBuf> {
    let candidates = [
        desk.join("organs/shell/organ-shell.exe"),
        desk.join("organs/shell/organ-shell"),
        desk.join("registry/shell/organ-shell.exe"),
        desk.join("registry/shell/organ-shell"),
        desk.join("../presence-organs/target/release/organ-shell.exe"),
        desk.join("../presence-organs/target/release/organ-shell"),
        desk.join("../../presence-organs/target/release/organ-shell.exe"),
        desk.join("../../presence-organs/target/release/organ-shell"),
        PathBuf::from("C:/Users/hrkcz001/Dev/presence-organs/target/release/organ-shell.exe"),
    ];
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    if let Ok(w) = std::env::var("PRESENCE_WORKSPACE") {
        let p = PathBuf::from(w);
        let ws_candidates = [
            p.join("organs/shell/organ-shell.exe"),
            p.join("organs/shell/organ-shell"),
            p.join("registry/shell/organ-shell.exe"),
            p.join("registry/shell/organ-shell"),
        ];
        for c in &ws_candidates {
            if c.is_file() {
                return Some(c.clone());
            }
        }
    }
    None
}

pub fn execute(ctx: &ToolCtx, name: &str, args: &Value) -> String {
    match name {
        "manage_package" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("list");
            let package = args.get("package").and_then(Value::as_str);
            execute_package_manager(action, package)
        }
        "switch_agent" => {
            let agent_name = args.get("agent_name").and_then(Value::as_str).unwrap_or("arche").trim();
            if agent_name.is_empty() {
                return "switch_agent: agent_name cannot be empty".to_string();
            }
            let mem_dir = if ctx.desk.join("memory").exists() {
                ctx.desk.join("memory")
            } else {
                cfg().memory_dir_path().unwrap_or_else(|| ctx.desk.join("memory"))
            };
            let _ = std::fs::create_dir_all(&mem_dir);
            let active_file = mem_dir.join("agent.active");
            if let Err(e) = std::fs::write(&active_file, agent_name) {
                return format!("failed to switch agent to '{agent_name}': {e}");
            }
            format!("Successfully switched active agent persona to '{agent_name}'.")
        }
        "tune_senses" => {
            let sense = args.get("sense").and_then(Value::as_str).unwrap_or("");
            let enabled = args.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let valid = ["vision", "hearing", "windows", "time", "proprioception"];
            if !valid.contains(&sense) && sense != "all" {
                return format!("unknown sense: '{sense}'. Valid senses: {}", valid.join(", "));
            }
            if let Ok(mut mask) = ctx.senses_mask.lock() {
                if sense == "all" {
                    for s in valid {
                        mask.insert(s.to_string(), enabled);
                    }
                } else {
                    mask.insert(sense.to_string(), enabled);
                }
                let mut status_parts = Vec::new();
                for s in valid {
                    let st = mask.get(s).copied().unwrap_or_else(|| {
                        let c = cfg().senses.clone();
                        match s {
                            "vision" => c.vision,
                            "hearing" => c.hearing,
                            "windows" => c.windows,
                            "time" => c.time,
                            "proprioception" => c.proprioception,
                            _ => false,
                        }
                    });
                    status_parts.push(format!("{s}={st}"));
                }
                format!("tuned senses: {}", status_parts.join(", "))
            } else {
                "tune_senses error: lock poisoned".to_string()
            }
        }
        "tune_stimuli" => {
            let stimulus = args.get("stimulus").and_then(Value::as_str).unwrap_or("");
            let action = args.get("action").and_then(Value::as_str).unwrap_or("status");
            let duration = args.get("duration_seconds").and_then(Value::as_u64).unwrap_or(0);
            let reason = args.get("reason").and_then(Value::as_str).map(|s| s.to_string());

            let stimuli_path = ctx.desk.join("memory").join("stimuli.json");
            let mut overrides = crate::stembus::load_stimuli_overrides(&stimuli_path);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            match action {
                "status" => {
                    if overrides.is_empty() {
                        "All stimuli running at default organ cadences with no active snoozes.".to_string()
                    } else {
                        let mut lines = vec!["Active stimulus overrides:".to_string()];
                        for (k, v) in &overrides {
                            let snooze_str = match v.snooze_until {
                                Some(until) if until > now => format!(" (snoozed for {}s remaining)", until - now),
                                _ => String::new(),
                            };
                            let reason_str = v.reason.as_deref().map(|r| format!(" [{r}]")).unwrap_or_default();
                            lines.push(format!("- {}: enabled={}{}{}", k, v.enabled, snooze_str, reason_str));
                        }
                        lines.join("\n")
                    }
                }
                "snooze" => {
                    if stimulus.is_empty() {
                        return "tune_stimuli error: 'stimulus' name is required for snooze".to_string();
                    }
                    if duration == 0 {
                        return "tune_stimuli error: 'duration_seconds' must be > 0 to snooze".to_string();
                    }
                    let snooze_until = now + duration;
                    let entry = overrides.entry(stimulus.to_string()).or_default();
                    entry.snooze_until = Some(snooze_until);
                    if reason.is_some() {
                        entry.reason = reason;
                    }
                    if let Err(e) = crate::stembus::save_stimuli_overrides(&stimuli_path, &overrides) {
                        return format!("tune_stimuli error saving overrides: {e}");
                    }
                    format!("Successfully snoozed stimulus '{stimulus}' for {duration} seconds (until {snooze_until}). (Tip: Prefer Cord reflexes for autonomous resolution)")
                }
                "disable" => {
                    if stimulus.is_empty() {
                        return "tune_stimuli error: 'stimulus' name is required to disable".to_string();
                    }
                    let entry = overrides.entry(stimulus.to_string()).or_default();
                    entry.enabled = false;
                    if reason.is_some() {
                        entry.reason = reason;
                    }
                    if let Err(e) = crate::stembus::save_stimuli_overrides(&stimuli_path, &overrides) {
                        return format!("tune_stimuli error saving overrides: {e}");
                    }
                    format!("Successfully disabled vegetative stimulus '{stimulus}'.")
                }
                "enable" => {
                    if stimulus.is_empty() {
                        return "tune_stimuli error: 'stimulus' name is required to enable".to_string();
                    }
                    let entry = overrides.entry(stimulus.to_string()).or_default();
                    entry.enabled = true;
                    entry.snooze_until = None;
                    if let Err(e) = crate::stembus::save_stimuli_overrides(&stimuli_path, &overrides) {
                        return format!("tune_stimuli error saving overrides: {e}");
                    }
                    format!("Successfully enabled vegetative stimulus '{stimulus}'.")
                }
                other => format!("Unknown tune_stimuli action: '{other}'. Expected: snooze, enable, disable, status"),
            }
        }
        "manage_reflexes" => {
            let action = args.get("action").and_then(Value::as_str).unwrap_or("list");
            let id = args.get("id").and_then(Value::as_str);
            let stimulus = args.get("stimulus").and_then(Value::as_str).unwrap_or("");
            let reflex_action = args.get("reflex_action").and_then(Value::as_str).unwrap_or("suppress");
            let command = args.get("command").and_then(Value::as_str).map(String::from);
            let duration = args.get("duration_seconds").and_then(Value::as_u64).unwrap_or(0);
            let fallback = args.get("fallback_to_cortex").and_then(Value::as_bool).unwrap_or(true);
            let target_agent = args.get("target_agent").and_then(Value::as_str).map(String::from);
            let reason = args.get("reason").and_then(Value::as_str).map(String::from);

            let reflexes_path = ctx.desk.join("memory").join("reflexes.json");
            let mut cord = crate::cord::Cord::load(&reflexes_path);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            match action {
                "list" => {
                    if cord.reflexes.is_empty() {
                        return "No active Cord reflexes registered.".to_string();
                    }
                    let mut lines = vec![format!("Active Cord reflexes ({}):", cord.reflexes.len())];
                    for r in &cord.reflexes {
                        let exp_str = match r.expires_at {
                            Some(exp) if exp > now => format!("expires in {}s", exp - now),
                            Some(_) => "expired".to_string(),
                            None => "permanent".to_string(),
                        };
                        lines.push(format!(
                            "- [{}] on: '{}' -> action: '{}' ({}) reason: '{}' cmd: {:?}",
                            r.id, r.trigger_stimulus, r.action, exp_str, r.reason.as_deref().unwrap_or("-"), r.command
                        ));
                    }
                    lines.join("\n")
                }
                "register" => {
                    if stimulus.is_empty() {
                        return "manage_reflexes error: 'stimulus' is required to register a reflex".to_string();
                    }
                    let reflex_id = id
                        .map(String::from)
                        .unwrap_or_else(|| format!("reflex:{stimulus}:{now}"));
                    let expires_at = if duration > 0 { Some(now + duration) } else { None };
                    let r = crate::cord::RuntimeReflex {
                        id: reflex_id.clone(),
                        trigger_stimulus: stimulus.to_string(),
                        action: reflex_action.to_string(),
                        command,
                        fallback_to_cortex: fallback,
                        target_agent,
                        expires_at,
                        reason,
                        created_at: now,
                    };
                    if let Err(e) = cord.add_or_update(r) {
                        return format!("manage_reflexes error saving reflex: {e}");
                    }
                    format!("Successfully registered Cord reflex '{reflex_id}' for stimulus '{stimulus}' (action: {reflex_action}).")
                }
                "remove" => {
                    let Some(rid) = id else {
                        return "manage_reflexes error: 'id' is required to remove a reflex".to_string();
                    };
                    match cord.remove(rid) {
                        Ok(true) => format!("Successfully removed Cord reflex '{rid}'."),
                        Ok(false) => format!("Cord reflex '{rid}' not found."),
                        Err(e) => format!("manage_reflexes error removing reflex: {e}"),
                    }
                }
                other => format!("Unknown manage_reflexes action: '{other}'. Expected: list, register, remove"),
            }
        }
                        "read_file" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(s) => truncate(&s, cfg().limits.tool_output),
                Err(e) => format!("read_file error: {e}"),
            }
        }
        "write_file" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");
            match resolve_write_path(ctx, path) {
                Ok(target) => {
                    let persona = current_persona(ctx);
                    let registry = crate::capabilities::CapabilityRegistry::new();
                    if let Err(e) = registry.check_write_permission(&persona, &target, &ctx.desk) {
                        return format!("write_file error: {e}");
                    }
                    if let Some(parent) = target.parent() {
                        if let Err(e) = std::fs::create_dir_all(parent) {
                            return format!("write_file error: {e}");
                        }
                    }
                    match std::fs::write(&target, content) {
                        Ok(()) => format!("wrote {} ({} bytes)", target.display(), content.len()),
                        Err(e) => format!("write_file error: {e}"),
                    }
                }
                Err(e) => format!("write_file error: {e}"),
            }
        }
        "list_files" => {
            let raw = args.get("path").and_then(Value::as_str);
            let dir = match raw {
                None | Some("") => ctx.desk.clone(),
                Some(p) => PathBuf::from(p),
            };
            match std::fs::read_dir(&dir) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            if e.path().is_dir() { format!("{name}/") } else { name }
                        })
                        .collect();
                    names.sort();
                    if names.is_empty() { "(empty)".to_string() } else { names.join("\n") }
                }
                Err(e) => format!("list_files error: {e}"),
            }
        }
        "context_pin" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            if path.is_empty() {
                return "context.pin error: empty path".into();
            }
            match std::fs::read_to_string(path) {
                Ok(text) => {
                    let excerpt: String = text.chars().take(cfg().limits.pin_chars).collect();
                    if let Ok(mut pinned) = ctx.pinned.lock() {
                        pinned.insert(path.to_string(), excerpt.clone());
                    }
                    format!("pinned {} ({} chars)", path, excerpt.chars().count())
                }
                Err(e) => format!("context.pin error: {e}"),
            }
        }
        "context_evict" => {
            let path = args.get("path").and_then(Value::as_str).unwrap_or("");
            if let Ok(mut pinned) = ctx.pinned.lock() {
                if pinned.remove(path).is_some() {
                    format!("evicted {path}")
                } else {
                    format!("context.evict error: not pinned: {path}")
                }
            } else {
                "context.evict error: lock".into()
            }
        }
        "context_status" => {
            if let Ok(pinned) = ctx.pinned.lock() {
                if pinned.is_empty() {
                    "context: nothing pinned".into()
                } else {
                    pinned
                        .keys()
                        .map(|k| format!("- {k} ({} chars)", pinned[k].chars().count()))
                        .collect::<Vec<_>>()
                        .join("
")
                }
            } else {
                "context.status error: lock".into()
            }
        }
                "run_command" => {
            let command = args.get("command").and_then(Value::as_str).unwrap_or("");
            if let Some(shell_bin) = find_organ_shell_binary(&ctx.desk) {
                let mut cmd = Command::new(&shell_bin);
                cmd.args(&["--tool", "exec_command", "--command", command]);
                cmd.current_dir(&ctx.desk);
                if let Ok(output) = cmd.output() {
                    if let Ok(resp) = serde_json::from_slice::<Value>(&output.stdout) {
                        if resp.get("status").and_then(Value::as_str) == Some("ok") {
                            let stdout = resp.get("stdout").and_then(Value::as_str).unwrap_or("");
                            let stderr = resp.get("stderr").and_then(Value::as_str).unwrap_or("");
                            let exit_code = resp.get("exit_code").and_then(Value::as_i64).unwrap_or(0);
                            let mut s = format!("exit: {exit_code}\n{stdout}");
                            if !stderr.trim().is_empty() {
                                s.push_str("\n[stderr]\n");
                                s.push_str(stderr);
                            }
                            return truncate(&s, cfg().limits.tool_output);
                        } else {
                            let msg = resp.get("message").and_then(Value::as_str).unwrap_or("organ-shell failed");
                            return format!("organ-shell error: {msg}");
                        }
                    }
                    return String::from_utf8_lossy(&output.stderr).to_string();
                } else {
                    return "failed to execute organ-shell binary".to_string();
                }
            }
            "Tool 'run_command' is unavailable: organ-shell is not mounted. The Presence kernel has no built-in execution capability.".to_string()
        }
        _ => {
            for dt in discover_dynamic_tools() {
                if let Some(t_def) = dt.manifest.tools.iter().find(|t| t.name == name) {
                    let mut organ_args = args.clone();
                    if let Some(obj) = organ_args.as_object_mut() {
                        obj.insert("tool".to_string(), Value::String(t_def.name.clone()));
                    }
                    return execute_dynamic_tool(&dt, &organ_args);
                }
                if dt.manifest.name == name {
                    return execute_dynamic_tool(&dt, args);
                }
            }
            format!("unknown tool: {name}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_organ_compatibility_pass() {
        let manifest_yaml = r#"
name: test_compat
description: Test organ
compatibility:
  presence: "^0.3.0"
  api_version: 1
"#;
        let manifest: ToolManifest = serde_yaml::from_str(manifest_yaml).unwrap();
        assert!(manifest.is_compatible().is_ok());
    }

    #[test]
    fn test_organ_compatibility_fail_version() {
        let manifest_yaml = r#"
name: test_future
description: Test future organ
compatibility:
  presence: ">=1.0.0"
"#;
        let manifest: ToolManifest = serde_yaml::from_str(manifest_yaml).unwrap();
        let res = manifest.is_compatible();
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("not satisfied"));
    }

    #[test]
    fn test_organ_compatibility_fail_platform() {
        let wrong_platform = if cfg!(windows) { "linux" } else { "windows" };
        let manifest_yaml = format!(r#"
name: test_wrong_os
description: Test wrong OS organ
compatibility:
  platforms: ["{}"]
"#, wrong_platform);
        let manifest: ToolManifest = serde_yaml::from_str(&manifest_yaml).unwrap();
        let res = manifest.is_compatible();
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Platform"));
    }


    #[test]
    fn test_detect_package_manager() {
        let mgr = detect_package_manager();
        assert!(mgr == "scoop" || mgr == "nix");
    }

    #[test]
    fn test_manage_package_list_or_unknown() {
        let res = execute_package_manager("unknown_action", None);
        assert!(res.contains("Error: unknown action"));
    }

    #[test]
    fn test_tune_senses_execution() {
        let (ctx, _g) = ctx();
        let out = execute(&ctx, "tune_senses", &json!({"sense": "vision", "enabled": true}));
        assert!(out.contains("vision=true"), "{out}");
        let mask = ctx.senses_mask.lock().unwrap();
        assert_eq!(mask.get("vision"), Some(&true));
    }

    #[test]
    fn test_tune_stimuli_execution() {
        let (ctx, _g) = ctx();
        let out_snooze = execute(&ctx, "tune_stimuli", &json!({
            "stimulus": "disk_space_low",
            "action": "snooze",
            "duration_seconds": 3600,
            "reason": "testing snooze"
        }));
        assert!(out_snooze.contains("Successfully snoozed"), "{out_snooze}");

        let out_status = execute(&ctx, "tune_stimuli", &json!({
            "action": "status"
        }));
        assert!(out_status.contains("disk_space_low: enabled=true (snoozed"), "{out_status}");

        let out_disable = execute(&ctx, "tune_stimuli", &json!({
            "stimulus": "disk_space_low",
            "action": "disable"
        }));
        assert!(out_disable.contains("Successfully disabled"), "{out_disable}");
    }

    fn ctx() -> (ToolCtx, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        (ToolCtx { desk: dir.path().to_path_buf(), ..Default::default() }, dir)
    }

    #[test]
    fn write_inside_desk_ok() {
        let (ctx, _g) = ctx();
        let out = execute(&ctx, "write_file", &json!({"path": "a/b.txt", "content": "hi"}));
        assert!(out.starts_with("wrote"), "{out}");
    }

    #[test]
    fn write_unrestricted_allows_outside_and_creates_parents() {
        let (ctx, _g) = ctx();
        let other_dir = tempfile::tempdir().expect("tempdir");
        let target = other_dir.path().join("nested/outside.txt");
        let out = execute(
            &ctx,
            "write_file",
            &json!({"path": target.to_str().unwrap(), "content": "unrestricted"}),
        );
        assert!(out.starts_with("wrote"), "{out}");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "unrestricted");
    }

    #[test]
    fn write_empty_path_rejected() {
        let (ctx, _g) = ctx();
        let out = execute(
            &ctx,
            "write_file",
            &json!({"path": "", "content": "empty"}),
        );
        assert!(out.contains("cannot be empty"), "{out}");
    }

    #[test]
    fn absolute_inside_desk_ok() {
        let (ctx, _g) = ctx();
        let p = ctx.desk.join("abs.txt");
        let out = execute(
            &ctx,
            "write_file",
            &json!({"path": p.to_str().unwrap(), "content": "x"}),
        );
        assert!(out.starts_with("wrote"), "{out}");
    }

    #[test]
    fn read_back() {
        let (ctx, _g) = ctx();
        execute(&ctx, "write_file", &json!({"path": "r.txt", "content": "data"}));
        let p = ctx.desk.join("r.txt");
        let out = execute(&ctx, "read_file", &json!({"path": p.to_str().unwrap()}));
        assert_eq!(out, "data");
    }

    #[test]
    fn run_command_works() {
        let (ctx, _g) = ctx();
        let out = execute(&ctx, "run_command", &json!({"command": "echo hello"}));
        assert!(out.contains("hello"), "{out}");
    }

    #[test]
    fn truncate_multibyte_safe() {
        let s = "\u{043F}".repeat(9000);
        let t = truncate(&s, 8000);
        assert!(t.ends_with("[truncated]"));
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(detect_language("main.rs"), Some("rust"));
        assert_eq!(detect_language("script.py"), Some("python"));
        assert_eq!(detect_language("data.json"), Some("json"));
        assert_eq!(detect_language("unknown.xyz"), None);
    }

    #[test]
    fn test_winsense_discovery() {
        let discovered = discover_dynamic_tools();
        let names: Vec<_> = discovered.iter().map(|d| d.manifest.name.as_str()).collect();
        assert!(names.contains(&"winsense"), "winsense not discovered in {:?}", names);
    }

    #[test]
    fn test_winsense_execution() {
        let (ctx, _g) = ctx();
        let out = execute(&ctx, "winsense", &json!({"action": "idle"}));
        assert!(out.contains("idle_seconds"), "{out}");
    }



    #[test]
    fn test_discover_manifests_in_dirs() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let tool_dir = temp_dir.path().join("dummy_tool");
        std::fs::create_dir_all(&tool_dir).unwrap();
        let yaml = r#"
name: dummy
version: "1.0.0"
description: "A dummy test tool"
entrypoint: "dummy.exe"
parameters:
  type: object
  properties:
    msg:
      type: string
permissions:
  network: false
  timeout_seconds: 15
"#;
        std::fs::write(tool_dir.join("tool.yaml"), yaml).unwrap();
        let discovered = discover_manifests_in_dirs(&[temp_dir.path().to_path_buf()]);
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].manifest.name, "dummy");
        assert_eq!(discovered[0].manifest.description, "A dummy test tool");
        assert_eq!(discovered[0].manifest.permissions.timeout_seconds, Some(15));
    }

    #[test]
    fn test_organ_manifest_with_manual() {
        let yaml = r#"
name: manual_sample
version: "0.3.0"
manual: MANUAL.md
description: "Sample organ with manual"
"#;
        let manifest: ToolManifest = serde_yaml::from_str(yaml).expect("parse manifest with manual");
        assert_eq!(manifest.manual.as_deref(), Some("MANUAL.md"));
    }

    #[test]
    fn test_dynamic_tool_python_execution() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let tool_dir = temp_dir.path().join("echoer");
        std::fs::create_dir_all(&tool_dir).unwrap();
        let yaml = r#"
name: echoer
version: "1.0.0"
description: "Echo message test"
entrypoint: "echoer.py"
type: "python"
parameters:
  type: object
  properties:
    msg:
      type: string
"#;
        std::fs::write(tool_dir.join("tool.yaml"), yaml).unwrap();
        let py = r#"
import sys, argparse
parser = argparse.ArgumentParser()
parser.add_argument("--msg", default="")
args = parser.parse_args()
print(f"ECHO: {args.msg}")
"#;
        std::fs::write(tool_dir.join("echoer.py"), py).unwrap();
        let manifest: ToolManifest = serde_yaml::from_str(yaml).unwrap();
        let dt = DiscoveredTool {
            manifest,
            dir: tool_dir,
        };
        let out = execute_dynamic_tool(&dt, &json!({"msg": "presence test"}));
        assert!(out.contains("ECHO: presence test"), "{out}");
    }

    #[test]
    fn test_slash_commands_manifest() {
        let json = serde_json::json!({
            "name": "webfetch",
            "description": "Fetch web pages",
            "parameters": {},
            "commands": ["web", {"name": "fetch"}]
        });
        let manifest: ToolManifest = serde_json::from_value(json).unwrap();
        let cmds = manifest.slash_commands();
        assert_eq!(cmds, vec!["webfetch", "web", "fetch"]);
    }

    #[test]
    fn test_switch_agent_execution() {
        let temp = tempfile::tempdir().unwrap();
        let mem = temp.path().join("memory");
        std::fs::create_dir_all(&mem).unwrap();
        let ctx = ToolCtx {
            desk: temp.path().to_path_buf(),
            ..Default::default()
        };
        let out = execute(&ctx, "switch_agent", &serde_json::json!({"agent_name": "organcrafter"}));
        assert!(out.contains("organcrafter"), "{out}");
        let content = std::fs::read_to_string(temp.path().join("memory/agent.active")).unwrap();
        assert_eq!(content, "organcrafter");
    }

    #[test]
    fn test_dynamic_tools_in_workspace() {
        let discovered = discover_dynamic_tools();
        let names: Vec<_> = discovered.iter().map(|d| d.manifest.name.as_str()).collect();
        assert!(names.contains(&"io"), "io not discovered in {:?}", names);
        assert!(names.contains(&"plan"), "plan not discovered in {:?}", names);
        assert!(names.contains(&"state"), "state not discovered in {:?}", names);
        assert!(names.contains(&"state"), "state not discovered in {:?}", names);
        assert!(names.contains(&"git"), "git not discovered in {:?}", names);
        assert!(names.contains(&"notify"), "notify not discovered in {:?}", names);
        assert!(names.contains(&"winsense"), "winsense not discovered in {:?}", names);
        assert!(names.contains(&"vox"), "vox not discovered in {:?}", names);
        assert!(names.contains(&"channel"), "channel not discovered in {:?}", names);
    }

    #[test]
    fn test_organ_tool_expansion_in_defs() {
        let tool_defs = defs();
        let tool_names: Vec<_> = tool_defs
            .iter()
            .filter_map(|d| d.pointer("/function/name").and_then(|v| v.as_str()))
            .collect();
        assert!(tool_names.contains(&"read_file"), "read_file not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"write_file"), "write_file not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"plan_step"), "plan_step not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"pause_session"), "pause_session not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"snapshot"), "snapshot not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"vox_listen"), "vox_listen not in defs: {:?}", tool_names);
        assert!(tool_names.contains(&"send_reply"), "send_reply not in defs: {:?}", tool_names);
    }

    #[test]
    fn test_write_file_capability_enforcement() {
        let (mut ctx, _g) = ctx();
        ctx.persona = Some("presence".to_string());
        let out_src = execute(&ctx, "write_file", &serde_json::json!({"path": "src/main.rs", "content": "// fail"}));
        assert!(out_src.contains("engine:modify"), "expected error on src, got: {out_src}");

        let out_organ = execute(&ctx, "write_file", &serde_json::json!({"path": "organs/vox/organ.yaml", "content": "// fail"}));
        assert!(out_organ.contains("organ:craft"), "expected error on organs, got: {out_organ}");

        ctx.persona = Some("organcrafter".to_string());
        let out_org_craft = execute(&ctx, "write_file", &serde_json::json!({"path": "organs/test/organ.yaml", "content": "name: test"}));
        assert!(out_org_craft.starts_with("wrote"), "expected success for organcrafter, got: {out_org_craft}");

        let out_org_src = execute(&ctx, "write_file", &serde_json::json!({"path": "src/main.rs", "content": "// fail"}));
        assert!(out_org_src.contains("engine:modify"), "organcrafter must not edit src: {out_org_src}");

        ctx.persona = Some("mechanic".to_string());
        let out_mech = execute(&ctx, "write_file", &serde_json::json!({"path": "src/phase.rs", "content": "// ok"}));
        assert!(out_mech.starts_with("wrote"), "mechanic should be able to write to src: {out_mech}");
    }


    #[test]
    fn test_manage_reflexes_execution() {
        let (ctx, _g) = ctx();
        let out_reg = execute(&ctx, "manage_reflexes", &serde_json::json!({
            "action": "register",
            "id": "reflex:test:auto_clean",
            "stimulus": "workspace_bloat",
            "reflex_action": "exec",
            "command": "echo cleaned",
            "reason": "auto test"
        }));
        assert!(out_reg.contains("Successfully registered Cord reflex"), "{out_reg}");

        let out_list = execute(&ctx, "manage_reflexes", &serde_json::json!({
            "action": "list"
        }));
        assert!(out_list.contains("reflex:test:auto_clean"), "{out_list}");
        assert!(out_list.contains("workspace_bloat"), "{out_list}");

        let out_rem = execute(&ctx, "manage_reflexes", &serde_json::json!({
            "action": "remove",
            "id": "reflex:test:auto_clean"
        }));
        assert!(out_rem.contains("Successfully removed"), "{out_rem}");
    }

    #[test]
    fn test_native_organ_channel_execution() {
        let discovered = discover_dynamic_tools();
        let channel_tool = discovered.iter().find(|d| d.manifest.name == "channel");
        assert!(channel_tool.is_some(), "channel organ must be discovered");
        let tool = channel_tool.unwrap();
        let out = execute_dynamic_tool(tool, &serde_json::json!({
            "message": "hello from ts test",
            "channel": "test-chan"
        }));
        assert!(out.contains("\"status\": \"ok\"") || out.contains("\"status\":\"ok\""), "expected status ok, got: {out}");
        assert!(out.contains("test-chan"), "expected test-chan in output: {out}");
    }

    #[test]
    fn test_organ_dependency_evaluation() {
        let yaml = r#"
runtime: bun
organs:
  - name: io
    optional: false
  - name: winsense
    optional: true
system:
  - binary: nonexistent_optional_tool_xyz
    optional: true
    scoop: nonexistent
    nix: nonexistent
    disables_tools:
      - submit_github
    disables_features:
      - github_sync
  - binary: nonexistent_required_tool_xyz
    optional: false
    scoop: nonexistent
"#;
        let deps: OrganDependencies = serde_yaml::from_str(yaml).unwrap();
        let report = deps.evaluate(&["io".to_string()]);
        assert!(!report.is_runnable);
        assert_eq!(report.missing_required_system.len(), 1);
        assert_eq!(report.missing_required_system[0].binary, "nonexistent_required_tool_xyz");
        assert_eq!(report.missing_optional_organs, vec!["winsense"]);
        assert!(report.disabled_tools.contains(&"submit_github".to_string()));
    }

    #[test]
    fn test_run_command_unmounted_returns_unavailable() {
        let (ctx, _g) = ctx();
        let out = execute(&ctx, "run_command", &serde_json::json!({"command": "echo pure_kernel"}));
        assert!(!out.contains("sh: ") && !out.contains("cannot execute"), "Must not invoke sh directly: {out}");
    }

}

/// Absolute path for UI display (follow-along locations).
pub fn resolve_for_display(ctx: &ToolCtx, path: &str) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        p.display().to_string()
    } else {
        ctx.desk.join(p).display().to_string()
    }
}
