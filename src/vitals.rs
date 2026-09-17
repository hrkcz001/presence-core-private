//! Vitals thread (PLAN §10, §11.4). One process-wide sampler: every 60s
//! append a line to memory/vitals.jsonl, ring-capped to 500 lines.
//! Windows numbers via a single powershell spawn per tick. Failures
//! append {"error": ...} and never wake anyone.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TICK: Duration = Duration::from_secs(60);
const MAX_LINES: usize = 500;

fn now_ts() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Sample CPU%, mem%, battery via one powershell spawn.
/// Returns (cpu, mem, battery) — None where the probe failed.
fn sample() -> (Option<f64>, Option<f64>, Option<String>) {
    let script = r#"
$cpu = (Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average
$os = Get-CimInstance Win32_OperatingSystem
$mem = [math]::Round(100 - ($os.FreePhysicalMemory / $os.TotalVisibleMemorySize) * 100, 1)
$b = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
if ($b) { "$cpu`n$mem`n$($b.EstimatedChargeRemaining)%" } else { "$cpu`n$mem`nnone" }
"#;
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        _ => return (None, None, None),
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
    let cpu = lines.next().and_then(|l| l.parse::<f64>().ok());
    let mem = lines.next().and_then(|l| l.parse::<f64>().ok());
    let battery = lines.next().map(str::to_owned);
    (cpu, mem, battery)
}

/// Ring-cap the file: keep the last MAX_LINES lines.
fn cap_file(path: &Path) {
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= MAX_LINES {
        return;
    }
    let keep: Vec<&str> = lines[lines.len() - MAX_LINES..].to_vec();
    if let Ok(mut f) = std::fs::File::create(path) {
        for l in keep {
            let _ = writeln!(f, "{l}");
        }
    }
}

/// Spawn the vitals thread. Call once from main.
pub fn spawn(memory_dir: PathBuf) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let vitals_path = memory_dir.join("vitals.jsonl");
        let started = SystemTime::now();
        loop {
            let (cpu, mem, battery) = sample();
            let rec = match (cpu, mem) {
                (Some(c), Some(m)) => serde_json::json!({
                    "ts": now_ts(),
                    "cpu": c,
                    "mem": m,
                    "battery": battery,
                    "uptime_s": started.elapsed().map(|e| e.as_secs()).unwrap_or(0),
                }),
                _ => serde_json::json!({"ts": now_ts(), "error": "sample failed"}),
            };
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&vitals_path)
            {
                let _ = writeln!(f, "{rec}");
            }
            cap_file(&vitals_path);
            std::thread::sleep(TICK);
        }
    })
}

/// Tail of the vitals log for the Observation prompt (last n lines).
pub fn tail(memory_dir: &Path, n: usize) -> String {
    let path = memory_dir.join("vitals.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return "(no vitals yet)".into();
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_empty_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(tail(dir.path(), 5), "(no vitals yet)");
    }

    #[test]
    fn tail_returns_last_n() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("vitals.jsonl");
        std::fs::write(&p, "one\ntwo\nthree").expect("write");
        assert_eq!(tail(dir.path(), 2), "two\nthree");
    }

    #[test]
    fn cap_file_trims_to_max() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("vitals.jsonl");
        let body: String = (0..600).map(|i| format!("{i}\n")).collect();
        std::fs::write(&p, &body).expect("write");
        cap_file(&p);
        let after = std::fs::read_to_string(&p).unwrap();
        assert_eq!(after.lines().count(), MAX_LINES);
        assert!(after.starts_with("100\n"));
    }
}
