# Presence Core

> Autonomous biological AI runtime in Rust (Triad: Cortex, Stem, Cord).

## Architecture Reference
For deep architectural background, full history, and subsystem specifications, read:
- [`TOTAL_ARCHITECTURE_AND_SPECIFICATION.md`](TOTAL_ARCHITECTURE_AND_SPECIFICATION.md)

---

## Quickstart: How to Build and Run Right Now

### 1. Requirements
- Rust toolchain (`cargo`, `rustc` 1.80+)
- Windows or Linux
- Sister repository: `presence-organs-private` cloned alongside this repo (see step 3)

### 2. Build Core Runtime
```powershell
# Build both the Conscious Cortex (presence.exe) and the Vegetative Stem (stem.exe)
cargo build --release
```
Binaries will be produced at:
- `target/release/presence.exe` (The agent / cortex)
- `target/release/stem.exe` (The background autonomous heartbeat / stem)

### 3. Setup Organs (Tools, Senses, Stimuli)
The core looks for executable organs in `./organs/`.
In the sister repository `presence-organs-private`:
```powershell
cargo build --release
# Copy compiled organ binaries into ./organs/
# (e.g. organ-shell.exe, organ-vitals.exe, organ-ask.exe, organ-packager.exe)
```

### 4. Configure LLM Gateway
Edit `.config/presence.yaml`:
```yaml
models:
  - name: your-model-name # e.g. gpt-4o, deepseek-chat, or qwen
    context_limit: 65536
    max_output_tokens: 8192
    capabilities:
      tools: true
      parallel_tool_calls: true
    endpoints:
      - base_url: https://your-api-endpoint/v1 # or http://127.0.0.1:4000/v1
        api_keys:
          - "your-api-key-here"
```

### 5. Run the Triad
1. **Terminal 1 — Run Stem (Autonomic Nervous System, 0 tokens)**:
   ```powershell
   .\target\release\stem.exe
   ```
   Polls stimuli, modulates pulse (5s -> 30s), executes involuntary reflexes via `src/cord.rs`.

2. **Terminal 2 — Run Cortex (Conscious Agent)**:
   ```powershell
   .\target\release\presence.exe
   ```
   Executes the 4-phase Hermeneutic Circle (`Observation` -> `Plan` -> `Anticipation` -> `Execution`).

---

## Tinkering & Wire-Up Guide for Agent (Immediate Low-Hanging Fruit)

If you are an AI agent helping the owner get this running immediately without deep rewrites, focus on these 3 concrete hookups:

### 1. Decouple `src/tools.rs:1188` (`PRESENCE_USER_IDLE_SECS`)
- **Current state**: Hardcoded call to `crate::winsense::get_user_idle_seconds()`.
- **Quick fix**: Wrap it with `#[cfg(windows)]` or fallback to checking the mounted senses/environment instead of directly coupling core to Win32 FFI.

### 2. Connect Cord Reflex Triggering in `src/bin/stem.rs`
- **Current state**: `stem.rs` discovers stimuli via `StemBus` and evaluates reflexes via `cord.evaluate()`, but if no reflex intercepts, it logs to `alarms.jsonl`.
- **Quick tweak**: Ensure `memory/reflexes.json` exists with default matchers (e.g., auto-suppress trivial battery polls or trigger auto-clean).

### 3. Validate Manifest Discovery (`organs/` vs `crates/organs/`)
- Ensure your `organ.yaml` manifests match the binary names compiled by `presence-organs`. Core automatically parses disjunctive `any_of` dependencies.

Run tests:
```powershell
cargo test
```
