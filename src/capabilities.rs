//! Scoped Capabilities Engine for Presence.
//!
//! Enforces path-scoped filesystem access, protected core runtime immutability,
//! and role-based execution boundaries for specialized personas.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityDef {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaGrant {
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub fs_write_scopes: Vec<String>,
    #[serde(default)]
    pub prohibited_write_scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityCatalog {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub capabilities: HashMap<String, CapabilityDef>,
    #[serde(default)]
    pub personas: HashMap<String, PersonaGrant>,
}

impl CapabilityCatalog {
    pub fn default_embedded() -> Self {
        let mut capabilities = HashMap::new();
        capabilities.insert(
            "fs:read".to_string(),
            CapabilityDef {
                description: "Read files across workspace and system paths".into(),
            },
        );
        capabilities.insert(
            "fs:write".to_string(),
            CapabilityDef {
                description: "Write files scoped to explicit path patterns".into(),
            },
        );
        capabilities.insert(
            "proc:spawn".to_string(),
            CapabilityDef {
                description: "Execute bounded subprocess commands".into(),
            },
        );
        capabilities.insert(
            "organ:craft".to_string(),
            CapabilityDef {
                description: "Define, build, test, and package peripheral organs and tool manifests".into(),
            },
        );
        capabilities.insert(
            "organ:mount".to_string(),
            CapabilityDef {
                description: "Mount and bind peripheral organs into the active runtime".into(),
            },
        );
        capabilities.insert(
            "engine:modify".to_string(),
            CapabilityDef {
                description: "Modify core nervous triad (src/**, Cargo.toml, engine binaries)".into(),
            },
        );
        capabilities.insert(
            "engine:repair".to_string(),
            CapabilityDef {
                description: "Diagnose, test, and safely self-repair / evolve core runtime".into(),
            },
        );

        let mut personas = HashMap::new();

        personas.insert(
            "presence".to_string(),
            PersonaGrant {
                role: "autonomous conscious agent - Cortex, Stem, Cord orchestrator".into(),
                aliases: vec![],
                capabilities: vec!["fs:read".into(), "proc:spawn".into(), "organ:mount".into()],
                fs_write_scopes: vec!["workspace/**".into(), "memory/**".into()],
                prohibited_write_scopes: vec!["src/**".into(), "Cargo.toml".into(), "organs/**".into()],
            },
        );

        personas.insert(
            "organcrafter".to_string(),
            PersonaGrant {
                role: "peripheral artisan - crafts, tests, packages organs and tool bindings".into(),
                aliases: vec![],
                capabilities: vec![
                    "fs:read".into(),
                    "proc:spawn".into(),
                    "organ:craft".into(),
                    "organ:mount".into(),
                ],
                fs_write_scopes: vec![
                    "organs/**".into(),
                    "workspace/organs/**".into(),
                    "workspace/tools/**".into(),
                    "tools/**".into(),
                    "workspace/**".into(),
                    "memory/**".into(),
                    "bucket/**".into(),
                ],
                prohibited_write_scopes: vec!["src/**".into(), "Cargo.toml".into()],
            },
        );

        personas.insert(
            "mechanic".to_string(),
            PersonaGrant {
                role: "nervous triad architect & self-improver (Cortex, Stem, Cord maintenance & safe evolution)".into(),
                aliases: vec![],
                capabilities: vec![
                    "fs:read".into(),
                    "proc:spawn".into(),
                    "engine:modify".into(),
                    "engine:repair".into(),
                    "organ:mount".into(),
                ],
                fs_write_scopes: vec![
                    "src/**".into(),
                    "Cargo.toml".into(),
                    "workspace/**".into(),
                    "memory/**".into(),
                    ".config/**".into(),
                    "docs/**".into(),
                ],
                prohibited_write_scopes: vec![],
            },
        );

        Self {
            version: "1.0".to_string(),
            capabilities,
            personas,
        }
    }

    pub fn load_from_str(s: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }

    pub fn load_from_file_or_default(path: &Path) -> Self {
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(path) {
                if let Ok(cat) = Self::load_from_str(&content) {
                    return cat;
                }
            }
        }
        Self::default_embedded()
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityRegistry {
    catalog: CapabilityCatalog,
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        let cand = Path::new(".config/capabilities.yaml");
        let catalog = if cand.is_file() {
            CapabilityCatalog::load_from_file_or_default(cand)
        } else {
            CapabilityCatalog::default_embedded()
        };
        Self { catalog }
    }

    #[allow(dead_code)]
    pub fn with_catalog(catalog: CapabilityCatalog) -> Self {
        Self { catalog }
    }

    pub fn find_persona(&self, name: &str) -> Option<(&str, &PersonaGrant)> {
        let trimmed = name.trim().to_lowercase();
        for (key, grant) in &self.catalog.personas {
            if key.to_lowercase() == trimmed {
                return Some((key.as_str(), grant));
            }
            if grant.aliases.iter().any(|a| a.to_lowercase() == trimmed) {
                return Some((key.as_str(), grant));
            }
        }
        None
    }

    pub fn check_write_permission(
        &self,
        persona: &str,
        target_path: &Path,
        root_dir: &Path,
    ) -> Result<(), String> {
        let (canonical_persona, grant) = match self.find_persona(persona) {
            Some(res) => res,
            None => {
                if let Some(g) = self.catalog.personas.get("arche") {
                    ("arche", g)
                } else if let Some(g) = self.catalog.personas.get("presence") {
                    ("presence", g)
                } else if let Some((k, g)) = self.catalog.personas.iter().next() {
                    (k.as_str(), g)
                } else {
                    return Err("No persona configured in capability catalog".into());
                }
            }
        };

        // Normalize paths for consistent cross-platform comparison
        let root_str = root_dir.to_string_lossy().replace('\\', "/");
        let root_clean = root_str.trim_end_matches('/');

        let target_str = target_path.to_string_lossy().replace('\\', "/");
        let target_clean = target_str.trim_end_matches('/');

        let (rel_clean, is_inside_root) = if target_clean == root_clean {
            (".", true)
        } else if target_clean.starts_with(root_clean) && target_clean.as_bytes().get(root_clean.len()) == Some(&b'/') {
            let s = &target_clean[root_clean.len() + 1..];
            (s, true)
        } else {
            (target_clean.trim_start_matches('/'), false)
        };

        // Normalize any internal .. or . components
        let rel_normalized = normalize_path_str(rel_clean);

        // 1. Core runtime protection: src/ or Cargo.toml / Cargo.lock (when inside repo/root)
        let is_engine_core = is_inside_root && (
            rel_normalized.starts_with("src/")
                || rel_normalized == "src"
                || rel_normalized == "Cargo.toml"
                || rel_normalized == "Cargo.lock"
        );

        if is_engine_core && !grant.capabilities.contains(&"engine:modify".to_string()) {
            return Err(format!(
                "permission denied: persona '{canonical_persona}' lacks capability 'engine:modify' required to write to '{rel_normalized}'"
            ));
        }

        // 2. Organ protection: organs/ (when inside repo/root)
        let is_organ_dir = is_inside_root && (rel_normalized.starts_with("organs/") || rel_normalized == "organs");
        if is_organ_dir
            && !grant.capabilities.contains(&"organ:craft".to_string())
            && !grant.capabilities.contains(&"engine:modify".to_string())
        {
            return Err(format!(
                "permission denied: persona '{canonical_persona}' lacks capability 'organ:craft' required to write to '{rel_normalized}'"
            ));
        }

        // 3. Check prohibited write scopes
        for prohibited in &grant.prohibited_write_scopes {
            if is_inside_root && matches_scope(prohibited, &rel_normalized) {
                return Err(format!(
                    "permission denied: persona '{canonical_persona}' is prohibited from writing to scope '{prohibited}' ('{rel_normalized}')"
                ));
            }
        }

        // 4. Check allowed write scopes if defined
        if is_inside_root && !grant.fs_write_scopes.is_empty() {
            let allowed = grant.fs_write_scopes.iter().any(|scope| matches_scope(scope, &rel_normalized));
            if !allowed && !grant.capabilities.contains(&"engine:modify".to_string()) {
                if is_engine_core || is_organ_dir {
                    return Err(format!(
                        "permission denied: persona '{canonical_persona}' write to '{rel_normalized}' outside allowed scopes: {:?}",
                        grant.fs_write_scopes
                    ));
                }
            }
        }

        Ok(())
    }
}

fn normalize_path_str(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    parts.join("/")
}

fn matches_scope(pattern: &str, path: &str) -> bool {
    let clean_pat = pattern.trim_start_matches("./");
    if clean_pat == "*" || clean_pat == "**" {
        return true;
    }
    if let Some(prefix) = clean_pat.strip_suffix("/**") {
        return path == prefix || path.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = clean_pat.strip_suffix("/*") {
        if path.starts_with(&format!("{prefix}/")) {
            let rest = &path[prefix.len() + 1..];
            return !rest.contains('/');
        }
        return false;
    }
    clean_pat == path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arche_cannot_modify_src_or_cargo_toml() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        let target = Path::new("C:/workspace/src/main.rs");
        let res = reg.check_write_permission("arche", target, root);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("engine:modify"));

        let target_cargo = Path::new("C:/workspace/Cargo.toml");
        let res_cargo = reg.check_write_permission("arche", target_cargo, root);
        assert!(res_cargo.is_err());
        assert!(res_cargo.unwrap_err().contains("engine:modify"));
    }

    #[test]
    fn arche_cannot_modify_organs() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        let target = Path::new("C:/workspace/organs/vox/organ.yaml");
        let res = reg.check_write_permission("arche", target, root);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("organ:craft"));
    }

    #[test]
    fn arche_can_write_agents_workspace_and_memory() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        assert!(reg.check_write_permission("arche", Path::new("C:/workspace/agents/researcher.agent.md"), root).is_ok());
        assert!(reg.check_write_permission("arche", Path::new("C:/workspace/workspace/doc.md"), root).is_ok());
        assert!(reg.check_write_permission("arche", Path::new("C:/workspace/memory/journal.md"), root).is_ok());
    }

    #[test]
    fn arbiter_can_write_workspace_and_memory_but_not_organs() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        assert!(reg.check_write_permission("arbiter", Path::new("C:/workspace/workspace/audit.md"), root).is_ok());
        assert!(reg.check_write_permission("arbiter", Path::new("C:/workspace/memory/incident.md"), root).is_ok());

        let res_organ = reg.check_write_permission("arbiter", Path::new("C:/workspace/organs/vox/organ.yaml"), root);
        assert!(res_organ.is_err());
    }

    #[test]
    fn organcrafter_can_write_organs_but_not_src() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        assert!(reg.check_write_permission("organcrafter", Path::new("C:/workspace/organs/new_organ/organ.yaml"), root).is_ok());
        let res = reg.check_write_permission("organcrafter", Path::new("C:/workspace/src/lib.rs"), root);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("engine:modify"));
    }

    #[test]
    fn mechanic_can_modify_triad_core() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        assert!(reg.check_write_permission("mechanic", Path::new("C:/workspace/src/phase.rs"), root).is_ok());
        assert!(reg.check_write_permission("mechanic", Path::new("C:/workspace/src/bin/stem.rs"), root).is_ok());
        assert!(reg.check_write_permission("mechanic", Path::new("C:/workspace/src/friction.rs"), root).is_ok());
        assert!(reg.check_write_permission("mechanic", Path::new("C:/workspace/Cargo.toml"), root).is_ok());
    }

    #[test]
    fn path_traversal_detection() {
        let reg = CapabilityRegistry::new();
        let root = Path::new("C:/workspace");
        let target = Path::new("C:/workspace/workspace/../src/main.rs");
        let res = reg.check_write_permission("arche", target, root);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("engine:modify"));
    }
}
