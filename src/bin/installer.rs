//! presence installer (owner directive 2026-09-15, plan:
//! workspace/presence-installer-plan.md).
//!
//! Interactive CLI, Russian, terse. Copies binaries + seed next to a
//! fresh workspace, writes a self-describing config and a manifest.
//! Step registry: each step is one struct with run(ctx).
//!
//! Platform-agnostic (Linux/Windows): home from the OS, binary
//! suffix from the OS, forward slashes in yaml. No editor-specific
//! glue — the client block speaks generic ACP; any ACP-capable
//! editor connects by pointing it at the presence binary.
//!
//! Distribution layout the installer expects (assembled by package.ps1):
//!
//!   <dist>/
//!     installer(.exe)
//!     bin/  presence converse ear ear_stream welcome heartbeat
//!     seed/ AGENTS.md MACHINE.md README.md agents/ governance/ knowledge/ ...
//!     tools/ presence/prompt.md, presence/bridge/proxy.mjs

use serde_json::json;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------- step registry ----------

enum StepOut {
    Done,
    Instructions(String),
    #[allow(dead_code)]
    Skipped(&'static str),
}

struct Ctx {
    dist_root: PathBuf, // where the installer binary lives
    workspace: PathBuf,
    desk: PathBuf,
    bin_dir: PathBuf,
    steps_done: Vec<&'static str>,
    instructions: Vec<String>,
}

trait Step {
    fn name(&self) -> &'static str;
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String>;
}

// ---------- helpers (platform-agnostic) ----------

fn ask(label: &str, default: &str) -> String {
    print!("{label} [{default}]: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line).unwrap_or(0);
    let t = line.trim();
    if t.is_empty() { default.to_string() } else { t.to_string() }
}

fn home() -> PathBuf {
    #[cfg(windows)]
    let h = std::env::var("USERPROFILE").unwrap_or_default();
    #[cfg(not(windows))]
    let h = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(h)
}

fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix('~') {
        return home().join(rest.trim_start_matches(['/', '\\']));
    }
    PathBuf::from(p)
}

/// Platform binary name: presence.exe / presence.
fn bin_name(name: &str) -> String {
    #[cfg(windows)]
    return format!("{name}.exe");
    #[cfg(not(windows))]
    name.to_string()
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("mkdir {}: {e}", dst.display()))?;
    for entry in std::fs::read_dir(src).map_err(|e| format!("read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

fn yaml_path(p: &Path) -> String {
    // forward slashes + quoting: portable yaml, safe on both OSes
    let s = p.to_string_lossy().replace('\\', "/");
    format!("\"{}\"", s.replace('"', ""))
}

// ---------- steps ----------

struct WorkspaceStep;
impl Step for WorkspaceStep {
    fn name(&self) -> &'static str { "workspace" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let d = home().join(".presence").to_string_lossy().to_string();
        let p = expand(&ask("workspace (rules, memory, governance)", &d));
        if !p.is_dir() {
            std::fs::create_dir_all(&p).map_err(|e| format!("mkdir: {e}"))?;
        } else if p.join("AGENTS.md").is_file() {
            return Err(format!("{} already looks like a workspace — delete it or choose another path", p.display()));
        }
        let seed = ctx.dist_root.join("seed");
        if seed.is_dir() {
            copy_dir_all(&seed, &p)?;
        }
        let tools = ctx.dist_root.join("tools");
        if tools.is_dir() {
            copy_dir_all(&tools, &p.join("tools"))?;
        }
        let mem = p.join("memory");
        std::fs::create_dir_all(&mem).map_err(|e| format!("mkdir: {e}"))?;
        ctx.workspace = p.clone();
        println!("  ok: {}", p.display());
        Ok(StepOut::Done)
    }
}

/// install.json: version, paths, steps — the uninstall/verify
/// surface. Written before git so it lands in the initial commit.
struct ManifestStep;
impl Step for ManifestStep {
    fn name(&self) -> &'static str { "manifest" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let m = json!({
            "version": env!("CARGO_PKG_VERSION"),
            "date": now,
            "paths": {
                "workspace": ctx.workspace.to_string_lossy().replace('\\', "/"),
                "desk": ctx.desk.to_string_lossy().replace('\\', "/"),
                "bin": ctx.bin_dir.to_string_lossy().replace('\\', "/"),
            },
            "steps": [],
        });
        let path = ctx.workspace.join("install.json");
        std::fs::write(&path, serde_json::to_string_pretty(&m).unwrap())
            .map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("  ok: {}", path.display());
        Ok(StepOut::Done)
    }
}

/// git init + initial commit: the workspace is a repo from birth;
/// rollback is always git (AGENTS.md principle).
struct GitStep;
impl Step for GitStep {
    fn name(&self) -> &'static str { "git" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        if ctx.workspace.join(".git").is_dir() {
            return Ok(StepOut::Skipped("already a repo"));
        }
        let run = |args: &[&str]| -> Result<String, String> {
            let out = std::process::Command::new("git")
                .args(args)
                .current_dir(&ctx.workspace)
                .output()
                .map_err(|e| format!("git {}: {e}", args[0]))?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).to_string())
            } else {
                Err(format!("git {}: {}", args[0], String::from_utf8_lossy(&out.stderr).trim()))
            }
        };
        run(&["init"])?;
        let _ = run(&["add", "-A"]);
        // identity for this one commit only — never touch global git config
        run(&["-c", "user.name=presence-installer",
              "-c", "user.email=installer@presence.local",
              "commit", "-m", "install: presence workspace seed"])?;
        println!("  ok: repo + initial commit");
        Ok(StepOut::Done)
    }
}

struct DeskStep;
impl Step for DeskStep {
    fn name(&self) -> &'static str { "desk" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let d = home().join("desk").to_string_lossy().to_string();
        let p = expand(&ask("desk (user working files and artifacts)", &d));
        std::fs::create_dir_all(&p).map_err(|e| format!("mkdir: {e}"))?;
        ctx.desk = p.clone();
        println!("  ok: {}", p.display());
        Ok(StepOut::Done)
    }
}

struct BinariesStep;
impl Step for BinariesStep {
    fn name(&self) -> &'static str { "binaries" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let src = ctx.dist_root.join("bin");
        if !src.is_dir() {
            return Err("bin/ not found next to installer".into());
        }
        let dst = ctx.workspace.join("bin");
        std::fs::create_dir_all(&dst).map_err(|e| format!("mkdir: {e}"))?;
        for entry in std::fs::read_dir(&src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let to = dst.join(entry.file_name());
            std::fs::copy(entry.path(), &to).map_err(|e| format!("copy: {e}"))?;
        }
        ctx.bin_dir = dst.clone();
        println!("  ok: {}", dst.display());
        Ok(StepOut::Done)
    }
}

struct ConfigStep;
impl Step for ConfigStep {
    fn name(&self) -> &'static str { "config" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let cfg_dir = ctx.workspace.join(".config");
        std::fs::create_dir_all(&cfg_dir).map_err(|e| format!("mkdir: {e}"))?;
        let path = cfg_dir.join("presence.yaml");
        let y = format!(
            "# presence.yaml — written by installer {}\n\
             # Precedence: env var > this file > compiled defaults.\n\
             # API key stays env-only: PRESENCE_API_KEY.\n\n\
             model:\n  base_url: \"https://agentrouter.org/v1\"\n  name: \"glm-5.3\"\n\n\
             paths:\n  desk: {}\n  haven: {}\n  bridge: {}\n  memory_dir: {}\n",
            env!("CARGO_PKG_VERSION"),
            yaml_path(&ctx.desk),
            yaml_path(&ctx.workspace),
            yaml_path(&ctx.workspace.join("bridge/proxy.mjs")),
            yaml_path(&ctx.workspace.join("memory")),
        );
        std::fs::write(&path, y).map_err(|e| format!("write {}: {e}", path.display()))?;
        println!("  ok: {}", path.display());
        Ok(StepOut::Done)
    }
}

struct ClientStep;
impl Step for ClientStep {
    fn name(&self) -> &'static str { "client" }
    fn run(&self, ctx: &mut Ctx) -> Result<StepOut, String> {
        let cmd = ctx.bin_dir.join(bin_name("presence"));
        let block = format!(
            "== ACP Client ==\n\
             command:  {}\n\
             env:      PRESENCE_API_KEY=<api_key>\n\
             \x20         PRESENCE_BASE_URL=https://agentrouter.org/v1 (default)\n\
             \x20         (configured in presence.yaml — API key required)\n\
             connection: point your ACP editor/client to the presence binary",
            cmd.display(),
        );
        println!("{block}");
        Ok(StepOut::Instructions(block))
    }
}

struct PrimerStep;
impl Step for PrimerStep {
    fn name(&self) -> &'static str { "primer" }
    fn run(&self, _ctx: &mut Ctx) -> Result<StepOut, String> {
        println!(
            "== Usage Guide ==\n\
             \x20 presence      ACP server: connect as agent in your editor\n\
             \x20 workspace     core: rules, memory, state — do not delete\n\
             \x20 desk          workspace: user artifacts and output files\n\
             \x20 voice         vox / ear (optional tool)\n\
             \x20 pause/stop    /pause in agent chat"
        );
        Ok(StepOut::Done)
    }
}

// ---------- main ----------

fn main() {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dist_root = exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));

    let mut ctx = Ctx {
        dist_root,
        workspace: PathBuf::new(),
        desk: PathBuf::new(),
        bin_dir: PathBuf::new(),
        steps_done: Vec::new(),
        instructions: Vec::new(),
    };

    let steps: Vec<Box<dyn Step>> = vec![
        Box::new(WorkspaceStep),
        Box::new(DeskStep),
        Box::new(BinariesStep),
        Box::new(ConfigStep),
        Box::new(ManifestStep),
        Box::new(GitStep),
        Box::new(ClientStep),
        Box::new(PrimerStep),
    ];

    println!("== presence installer v{} ==", env!("CARGO_PKG_VERSION"));
    for step in &steps {
        println!("[{}]", step.name());
        match step.run(&mut ctx) {
            Ok(StepOut::Done) => ctx.steps_done.push(step.name()),
            Ok(StepOut::Instructions(t)) => {
                ctx.steps_done.push(step.name());
                ctx.instructions.push(t);
            }
            Ok(StepOut::Skipped(_)) => {}
            Err(e) => {
                println!("  FAIL: {e}");
                println!("installation cancelled");
                std::process::exit(1);
            }
        }
    }

println!("completed successfully.");
}
