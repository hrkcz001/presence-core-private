//! Native Rust packager for presence distribution (PLAN §2.6).
//! Builds all release binaries and bundles a self-contained distribution in `target/dist/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("=== Packaging Presence Distribution ===");
    let root = env_workspace_root();
    println!("Workspace root: {}", root.display());

    let crate_dir = if root.join("presence/Cargo.toml").exists() {
        root.join("presence")
    } else {
        root.clone()
    };
    println!("Crate directory: {}", crate_dir.display());

    let target_dist = root.join("target/dist");
    let dist_bin = target_dist.join("bin");
    let dist_tools = target_dist.join("tools");
    let dist_config = target_dist.join(".config");

    fs::create_dir_all(&dist_bin).expect("Failed to create dist/bin");
    fs::create_dir_all(&dist_tools).expect("Failed to create dist/tools");
    fs::create_dir_all(&dist_config).expect("Failed to create dist/.config");

    // 1. Build release binaries
    println!("Building binaries (release mode)...");
    let binaries = ["presence", "winsense", "heartbeat"];
    for bin in &binaries {
        println!("  - Compiling {bin}...");
        let status = Command::new("cargo")
            .args(["build", "--bin", bin, "--release"])
            .current_dir(&crate_dir)
            .status()
            .expect("Failed to execute cargo build");
        if !status.success() {
            eprintln!("Error: failed to build binary '{bin}'");
            std::process::exit(1);
        }
    }

    // 2. Copy binaries
    let target_release = crate_dir.join("target/release");
    for bin in &binaries {
        let exe_name = if cfg!(windows) {
            format!("{bin}.exe")
        } else {
            bin.to_string()
        };
        let src = target_release.join(&exe_name);
        let dst = dist_bin.join(&exe_name);
        println!("  - Copying {} -> {}", src.display(), dst.display());
        fs::copy(&src, &dst).unwrap_or_else(|e| {
            panic!("Failed to copy {}: {e}", src.display());
        });
    }

    // 2.5 Copy seed files into dist/seed
    let seed_dir = crate_dir.join("seed");
    let dist_seed = target_dist.join("seed");
    if seed_dir.is_dir() {
        println!("Copying seed directory from {}...", seed_dir.display());
        let _ = copy_dir_all(&seed_dir, &dist_seed);
    }

    // 3. Copy workspace/tools manifests
    let ws_tools = root.join("workspace/tools");
    if ws_tools.is_dir() {
        println!("Copying tools manifests from {}...", ws_tools.display());
        copy_dir_all(&ws_tools, &dist_tools).expect("Failed to copy workspace/tools");
    }

    // 4. Generate default config.yaml in dist/.config if not present
    let default_config = target_dist.join("config.yaml");
    let config_content = r#"# presence Standalone Configuration
paths:
  workspace: "."
  desk: "desk"
  memory_dir: "memory"
  tools_dir: "tools"

senses:
  time: true
  hearing: true
  vision: true
  proprioception: true
  windows: true

limits:
  tool_output: 10000
  transcript_tail: 20
"#;
    fs::write(&default_config, config_content).expect("Failed to write config.yaml");

    // 5. Generate Scoop manifests in dist
    let post_install_script = vec![
        r#"$ws = "$env:USERPROFILE\.presence";"#.to_string(),
        r#"if (!(Test-Path $ws)) { New-Item -ItemType Directory -Force $ws | Out-Null; }"#.to_string(),
        r#"if (!(Test-Path "$ws\tools")) { New-Item -ItemType Directory -Force "$ws\tools" | Out-Null; }"#.to_string(),
        r#"if (!(Test-Path "$ws\memory")) { New-Item -ItemType Directory -Force "$ws\memory" | Out-Null; }"#.to_string(),
        r#"if (!(Test-Path "$ws\.config")) { New-Item -ItemType Directory -Force "$ws\.config" | Out-Null; }"#.to_string(),
        r#"if (Test-Path "$dir\seed") { Copy-Item -Recurse -Force "$dir\seed\*" $ws; }"#.to_string(),
        "Write-Host 'Presence workspace initialized at' $ws -ForegroundColor Green;".to_string(),
    ];

    let scoop_json = serde_json::json!({
        "version": "0.3.0",
        "description": "Autonomous ACP Agent Server for Presence Workspace",
        "homepage": "https://github.com/hrkcz001/presence",
        "bin": [
            "bin\\presence.exe",
                        "bin\\winsense.exe",
            "bin\\heartbeat.exe"
        ],
        "persist": [".config", "memory"],
        "post_install": post_install_script
    });

    let scoop_manifest = target_dist.join("presence.json");
    fs::write(&scoop_manifest, serde_json::to_string_pretty(&scoop_json).unwrap()).unwrap();
    
    // Also copy scoop manifest to repo root
    let repo_scoop = root.join("presence.json");
    let _ = fs::write(&repo_scoop, serde_json::to_string_pretty(&scoop_json).unwrap());
    
    println!("\nDistribution successfully assembled at: {}", target_dist.display());
    println!("Scoop manifest written to: {}", repo_scoop.display());
}

fn env_workspace_root() -> PathBuf {
    if let Ok(w) = std::env::var("PRESENCE_WORKSPACE") {
        let p = PathBuf::from(w);
        if p.is_dir() { return p; }
    }
    if let Ok(h) = std::env::var("PRESENCE_WORKSPACE").or_else(|_| std::env::var("PRESENCE_WORKSPACE")) {
        let p = PathBuf::from(h);
        if p.is_dir() { return p; }
    }
    // Search upwards from current directory for repository root (.git or workspace/)
    let mut cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if cur.join(".git").exists() || (cur.join("workspace").exists() && cur.join("tools").exists()) {
            return cur;
        }
        if let Some(parent) = cur.parent() {
            cur = parent.to_path_buf();
        } else {
            break;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}
