// Quest journal (owner request): standing goals with priorities.
// The circle pulls ONE quest at a time (top priority); unclear
// ranking -> ask the owner. Journal lives in memory/quests.jsonl.

use serde_json::{json, Value};

fn path(memory_dir: &std::path::Path) -> std::path::PathBuf {
    memory_dir.join("quests.jsonl")
}

#[derive(Debug, Clone)]
pub struct Quest {
    pub id: u64,
    pub text: String,
    pub title: String, // RPG-style short name for the journal
    pub priority: i32, // lower = more important (1 highest)
    pub status: String, // open | in_progress | done | dropped
}

pub fn load(memory_dir: &std::path::Path) -> Vec<Quest> {
    let text = std::fs::read_to_string(path(memory_dir)).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let v: Value = serde_json::from_str(l).ok()?;
            Some(Quest {
                id: v.get("id").and_then(Value::as_u64).unwrap_or(0),
                text: v.get("text").and_then(Value::as_str).unwrap_or("").to_string(),
                title: v.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
                priority: v.get("priority").and_then(Value::as_i64).unwrap_or(100) as i32,
                status: v.get("status").and_then(Value::as_str).unwrap_or("open").to_string(),
            })
        })
        .collect()
}

pub fn save(memory_dir: &std::path::Path, quests: &[Quest]) {
    let body: String = quests
        .iter()
        .map(|q| {
            json!({"id": q.id, "text": q.text, "title": q.title, "priority": q.priority, "status": q.status}).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(path(memory_dir), body + "\n");
}

/// Next actionable quest: top priority among open/in_progress.
pub fn next(memory_dir: &std::path::Path) -> Option<Quest> {
    let mut open: Vec<Quest> = load(memory_dir)
        .into_iter()
        .filter(|q| q.status == "open" || q.status == "in_progress")
        .collect();
    open.sort_by_key(|q| q.priority);
    open.into_iter().next()
}

/// Add a quest; returns its id.
pub fn add(memory_dir: &std::path::Path, text: &str, title: &str, priority: i32) -> u64 {
    let mut quests = load(memory_dir);
    let id = quests.iter().map(|q| q.id).max().unwrap_or(0) + 1;
    quests.push(Quest { id, text: text.to_string(), title: title.to_string(), priority, status: "open".into() });
    save(memory_dir, &quests);
    id
}

pub fn set_status(memory_dir: &std::path::Path, id: u64, status: &str) {
    let mut quests = load(memory_dir);
    for q in quests.iter_mut() {
        if q.id == id {
            q.status = status.to_string();
        }
    }
    save(memory_dir, &quests);
}

/// The visible journal (for prompts and cards).
pub fn render(memory_dir: &std::path::Path) -> String {
    let mut qs = load(memory_dir);
    qs.sort_by_key(|q| (q.priority, q.id));
    if qs.is_empty() {
        return "(journal empty)".into();
    }
    qs.iter()
        .map(|q| {
            let mark = match q.status.as_str() {
                "done" => "[x]",
                "in_progress" => "[>]",
                "dropped" => "[-]",
                _ => "[ ]",
            };
            if q.title.is_empty() {
                format!("{mark} #{} P{} {}", q.id, q.priority, q.text)
            } else {
                format!("{mark} {} — P{}
      {}", q.title, q.priority, q.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Prompt for the model to (re)rank priorities when unclear.
pub fn rank_prompt(journal: &str) -> String {
    format!(
        "Here is the quest journal. Rank priorities (1 = highest priority, larger = less urgent) \
         and return ONLY a JSON array: [{{\"id\": N, \"priority\": P}}] for all open quests.

{journal}"
    )
}

/// Ask-the-owner prompt when even ranking is ambiguous.
pub fn ask_owner_prompt(journal: &str) -> String {
    format!(
        "Cannot determine the next quest unambiguously. \
         Formulate ONE concise question to the owner (under 15 words) asking which quest to undertake next.

{journal}"
    )
}

/// One cheap call: RPG-style naming for a quest.
/// Returns (title, description): title <= 6 words RPG-flavored for the
/// journal; description = the full internal quest text.
pub fn name_quest(bridge: &mut crate::llm::Bridge, url: &str, key: &str, model: &str, text: &str) -> (String, String) {
    let req = format!(
        "Formulate a title and concise description for the agent task journal. Task: \"{}\". \
         Return ONLY JSON: {{\"title\": \"...\", \"description\": \"...\"}}.",
        text
    );
    let mut msgs = vec![
        serde_json::json!({"role": "system", "content": "You name quests concisely. Output only JSON."}),
        serde_json::json!({"role": "user", "content": req}),
    ];
    let out = crate::agent_loop(
        bridge, url, key, model,
        &mut msgs, &vec![], std::sync::Arc::new(crate::tools::ToolCtx {
            desk: ".".into(), pinned: Default::default(), senses_mask: Default::default(),
        }), "naming", std::path::Path::new("nul"), "naming", 1,
    );
    if let Ok(t) = out {
        if let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t[a..=b]) {
                let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("Task").to_string();
                let desc = v.get("description").and_then(|x| x.as_str()).unwrap_or(text).to_string();
                return (title, desc);
            }
        }
    }
    (text.chars().take(40).collect(), text.to_string())
}
