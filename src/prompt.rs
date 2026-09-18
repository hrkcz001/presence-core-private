//! System prompt assembly. Constitution (live workspace files) + operational
//! part. main.rs calls build().

use std::path::Path;

fn read_if_exists(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}


pub fn active_persona_mounted_organs(root: &Path, persona: &str) -> Option<Vec<String>> {
    let agent_candidates = [
        format!("agents/{persona}.agent.md"),
        format!("seed/agents/{persona}.agent.md"),
    ];
    for cand in &agent_candidates {
        if let Some(content) = read_if_exists(&root.join(cand)) {
            // Check frontmatter between leading --- and next ---
            let lines: Vec<&str> = content.lines().collect();
            if lines.first().map(|l| l.trim()) == Some("---") {
                if let Some(end_idx) = lines[1..].iter().position(|l| l.trim() == "---") {
                    let frontmatter = lines[1..=end_idx].join("\n");
                    if let Ok(val) = serde_yaml::from_str::<serde_json::Value>(&frontmatter) {
                        if let Some(arr) = val.get("organs").and_then(|v| v.as_array()) {
                            let list = arr.iter().filter_map(|item| item.as_str().map(|s| s.to_string())).collect();
                            return Some(list);
                        }
                    }
                }
            }
        }
    }
    None
}

pub fn available_commands_for_persona(root: &Path, persona: &str) -> Vec<serde_json::Value> {
    use serde_json::json;

    let mut avail_cmds = vec![
        json!({"name": "pause", "description": "Pause circle immediately"}),
        json!({"name": "continue", "description": "Resume paused circle"}),
        json!({"name": "agent", "description": "Switch active persona (/agent <name>)"}),
        json!({"name": "agents", "description": "List available personas"}),
        json!({"name": "journal", "description": "Display quest journal and memory"}),
    ];

    let mounted_organs = active_persona_mounted_organs(root, persona);

    for dt in crate::tools::discover_dynamic_tools() {
        if let Some(mounted) = &mounted_organs {
            let is_mounted = mounted.iter().any(|m| {
                m.eq_ignore_ascii_case(&dt.manifest.name)
                    || format!("organ-{m}").eq_ignore_ascii_case(&dt.manifest.name)
                    || dt.manifest.name.eq_ignore_ascii_case(&format!("organ-{m}"))
            });
            if !is_mounted {
                continue;
            }
        }

        for (name, desc) in dt.manifest.command_descriptors() {
            if !avail_cmds.iter().any(|c| c.get("name").and_then(serde_json::Value::as_str) == Some(&name)) {
                avail_cmds.push(json!({
                    "name": name,
                    "description": desc,
                }));
            }
        }
    }

    avail_cmds
}

pub fn active_persona(root: &Path, memory_dir: &Path) -> String {
    if let Ok(s) = std::fs::read_to_string(memory_dir.join("agent.active")) {
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    if let Ok(s) = std::fs::read_to_string(root.join("memory/agent.active")) {
        let s = s.trim();
        if !s.is_empty() {
            return s.to_string();
        }
    }
    "arche".to_string()
}

/// Constitution: rules, machine notes, active agent card (embedding state), and
/// attached organ instructions discovered dynamically — read live from workspace.
fn constitution(root: &Path, memory_dir: &Path) -> String {
    let mut parts = Vec::new();
    let persona = active_persona(root, memory_dir);
    for f in ["AGENTS.md", "MACHINE.md"] {
        if let Some(s) = read_if_exists(&root.join(f)) {
            parts.push(format!("--- {f} ---\\n{s}"));
        }
    }
    let agent_candidates = [
        format!("agents/{persona}.agent.md"),
        format!("seed/agents/{persona}.agent.md"),
        "agents/arche.agent.md".to_string(),
        "seed/agents/arche.agent.md".to_string(),
    ];
    for cand in &agent_candidates {
        if let Some(s) = read_if_exists(&root.join(cand)) {
            parts.push(format!("--- {cand} ---\\n{s}"));
            break;
        }
    }

    let organs = crate::tools::discover_dynamic_tools();
    if !organs.is_empty() {
        let mut org_entries = Vec::new();
        for org in &organs {
            let mut entry = format!("- **{}**: {}", org.manifest.name, org.manifest.description);
            if let Some(instr) = &org.manifest.instructions {
                if !instr.trim().is_empty() {
                    entry.push_str(&format!("\n  Instructions: {}", instr.trim()));
                }
            }
            org_entries.push(entry);
        }
        parts.push(format!("--- attached organs ---\n{}", org_entries.join("\n\n")));
    }

    parts.join("\n\n")
}

fn operational_from_file(root: &Path, desk: &Path) -> Option<String> {
    let text = read_if_exists(&root.join("prompt.md"))
        .or_else(|| read_if_exists(&root.join("tools/presence/prompt.md")))
        .or_else(|| read_if_exists(&root.join("presence/prompt.md")))?;
    Some(text.replace("{DESK}", &desk.display().to_string()))
}

fn operational(desk: &Path) -> String {
    let mut s = String::new();
    s.push_str("--- session model ---\n");
    s.push_str(
        "You are Presence. Each conversation window is a session; your memory \
         is the workspace. Messages arrive tagged with their topic. \
         Reply in the user's language.\n\n",
    );
    s.push_str("--- response format ---\n");
    s.push_str(
        "The channel follows the nature of the request; you pick.\n\
         - Task: verdict first, key nuances, no unnecessary markdown walls. Large output \
         goes to workspace files; the chat gets the path + a concise summary.\n\
         - Conversation: direct, substantive answers without artificial compression or filler.\n\n",
    );
    s.push_str("--- workspace ---\n");
    s.push_str(&format!(
        "Working artifacts live in the workspace desk: {}.\n\n",
        desk.display()
    ));
    s.push_str("--- tools and conduct ---\n");
    s.push_str(
        "Tools: read_file, list_files, write_file, run_command, tune_senses, manage_package, switch_agent. \
         Your reasoning is private: the user sees your messages and tool calls, never your internal thoughts.",
    );
    s
}

pub fn build(root: &Path, desk: &Path, memory_dir: &Path) -> String {
    let op = operational_from_file(root, desk).unwrap_or_else(|| operational(desk));
    format!("{}\n\n{}", constitution(root, memory_dir), op)
}
