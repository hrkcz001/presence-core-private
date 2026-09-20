# PRESENCE: Total Architectural Specification & Project Encyclopedia

**Version:** 1.0-Comprehensive  
**Date of Audit & Synthesis:** 2026-09-20  
**Target Audience:** System Architects, Autonomous Agents, AI Auditors, Kernel Engineers  
**Repositories:**
1. `haven` (`C:/Users/hrkcz001/haven`) — Governance, Personas, Architecture Plans, Knowledge Hub
2. `presence` (`C:/Users/hrkcz001/Dev/presence`) — Core Autonomous Runtime, Triad Kernel, Capabilities
3. `presence-organs` (`C:/Users/hrkcz001/Dev/presence-organs`) — Peripheral Organs Catalog, Rust SDK, ACP Gateway, Dashboard

---

## 1. Genesis & Historical Evolution

### 1.1 The Dasein Era (2026-09-12 – 2026-09-16)
Presence began under the codename **Dasein** (inspired by Heidegger's philosophy of being-in-the-world, thrownness *Geworfenheit*, and breakdown of equipment *Unzuhandenheit*).
- **M0**: Initial echo ACP (Agent Client Protocol) server in Rust communicating over stdio with Zed.
- **M1**: Live LLM bridge through `proxy.mjs` (a Node.js TLS-fingerprint proxy) connecting to `agentrouter.org`. Multi-turn topic memory.
- **M2**: Fundamental system tools: desk-scoped file editing, bounded CLI execution (`run_command`), and token ledger tracking.
- **M3**: Hermeneutic Circle state machine (`Situation` -> `Plan` -> `Anticipation` -> `Execution/Vollzug`). Ring buffer hardware telemetry (`vitals.rs`), friction tracking (`friction.rs`), and mood derivation.
- **M4**: Context budgeting (`budgeter.rs`): token estimation, priority-based prompt assembly, and pre-flight compaction fallback.
- **M5**: Real-time audio perceptual stack (`tools/dasein-voice`): continuous speech-to-text (`vosk` + `ffmpeg`), neural TTS (`edge-tts`), and conversation loops (`converse.exe`).

### 1.2 The Presence Renaming & Triad Decoupling (2026-09-17)
On 2026-09-17, the system underwent a radical architectural purification:
1. **Name & Identity**: Dasein was renamed to **Presence**. Russian UI/card hardcodes were purged, and Heideggerian philosophical jargon was eliminated from model-facing prompts to prevent LLM hallucinations.
2. **Repository Decoupling**: The runtime was extracted from `haven/tools/dasein` into a dedicated repository `C:/Users/hrkcz001/Dev/presence`. Community/peripheral organs were split into `C:/Users/hrkcz001/Dev/presence-organs`.
3. **The Biological Nervous Triad**: Replaced ad-hoc daemon loops with a strict biological triad: **Cortex** (conscious LLM cognition), **Stem** (autonomic vegetative pulse), and **Cord** (involuntary spinal reflexes).
4. **The Five-Faceted Organ Contract**: Organs were redefined from simple CLI tools into 5 distinct entities: `tool`, `sense`, `stimulus`, `reflex`, and `action`.
5. **Two-Tier Stack**: Python was totally expunged. All organs are strictly **pure Rust** (high performance, OS FFI) or **TypeScript on Bun** (scripting, rapid prototyping).

---

## 2. Fundamental Philosophy & Biological Triad

```
                                  =================================================
                                  |         CORTEX (Conscious Cognition)          |
                                  |  src/main.rs, src/daemon.rs, src/phase.rs     |
                                  |  - Hermeneutic Circle (Observe->Plan->Act)    |
                                  |  - Prompt Assembly: pulls SENSES (senses.rs)  |
                                  |  - Function Calling: invokes TOOLS (tools.rs) |
                                  |  - Token Cost: EXPENSIVE (LLM API Calls)      |
                                  =================================================
                                                    ^             |
                                      Invites wake  |             |  Deploys
                                   via alarms.jsonl |             |  reflex rules
                                                    |             v
                                  =================================================
                                  |            STEM (Vegetative Homeostasis)      |
                                  |  src/bin/stem.rs, src/stembus.rs              |
                                  |  - Autonomous out-of-band heartbeat loop      |
                                  |  - Polls STIMULI via StemBus on cadences      |
                                  |  - Pulse modulation: 5s (active) -> 30s/60s   |
                                  |  - Token Cost: ZERO (Local OS execution)      |
                                  =================================================
                                                          |
                                           Triggered      |  Involuntary
                                           stimulus event |  interception
                                                          v
                                  =================================================
                                  |             CORD (Spinal Reflex Arc)          |
                                  |  src/cord.rs, memory/reflexes.json            |
                                  |  - Sub-millisecond deterministic protection   |
                                  |  - Suppress alarm / Execute fast repair cmd   |
                                  |  - Token Cost: ZERO                           |
                                  =================================================
```

### 2.1 Cortex (The Conscious Brain)
- **Role**: High-level reasoning, strategy, multi-phase problem solving, and human dialogue.
- **Engine**: The Hermeneutic Circle (`src/phase.rs`, `src/daemon.rs`).
- **Data Inflow**: Senses (`senses.rs`) synchronously gathered at prompt construction.
- **Data Outflow**: Tools (`tools.rs`) executed in response to LLM tool calls.
- **Cost**: Expensive in monetary cost and latency. Cortex must NEVER be woken for routine telemetry checks.

### 2.2 Stem (The Autonomic Vegetative Brainstem)
- **Role**: Maintains physiological homeostasis, surveys physical presence, hardware vitals, and network status.
- **Engine**: `src/bin/stem.rs` and `src/stembus.rs`.
- **Data Inflow**: Stimuli polled at organ-defined cadences (`cadence_secs`).
- **Autonomous Behaviors**:
  1. **Pulse Modulation**: Adjusts loop sleep interval based on user activity (e.g. 5s baseline -> 30s user idle -> 60s screen lock).
  2. **Brainless Mode**: If `network_state` reports offline, cuts off LLM calls entirely and drops into an autonomous survival loop.
  3. **Invitation Escalation**: When homeostatic thresholds fail and cannot be reflexively solved, writes a structured alarm into `memory/alarms.jsonl` to invite Cortex to wake up.
- **Cost**: Zero tokens. Pure native subprocess execution.

### 2.3 Cord (The Spinal Reflex Engine)
- **Role**: Rapid, involuntary protective reactions.
- **Engine**: `src/cord.rs`, configured dynamically via `memory/reflexes.json`.
- **Mechanism**: Intercepts triggered stimuli from `StemBus` *before* they are written to `alarms.jsonl`.
- **Outcomes**:
  - `Suppressed`: Explicitly ignores known false positives (e.g. "ignore low disk space until tomorrow morning").
  - `Executed`: Runs a deterministic shell command in <1ms (e.g. `cargo clean --target-dir target/tmp` on workspace bloat). If successful, resolves the emergency with 0 tokens and no LLM interruption.
  - `Escalate`: Permits the stimulus to escalate into `alarms.jsonl` for Cortex attention.

---

## 3. The Five-Faceted Organ Contract

Every peripheral capability in Presence belongs to an **Organ** conforming to the five-faceted specification (`SENSES-VS-STIMULI-ONTOLOGY.md`):

| Facet | Consumer / Caller | Invocation Timing | Token Footprint | Execution Protocol | Example |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **`sense`** | **Cortex** | Synchronously before turn (`prompt::build`) | **High** (enters context prompt) | Read-only snapshot of state | `overview`, `current_mood`, `system_vitals` |
| **`stimulus`** | **Stem** | Asynchronously on background cadence | **Zero** | Periodic threshold polling CLI | `battery_critical`, `user_idle`, `git_dirty_drift` |
| **`reflex`** | **Cord** | Triggered by stimulus in Stem loop | **Zero** | Involuntary deterministic CLI command | `suppress`, `cargo clean target/tmp` |
| **`tool`** | **Cortex** | Conscious turn execution | **Variable** | JSON-RPC / CLI function calling | `io_write_file`, `exec_command`, `ask_question` |
| **`action`** | **User / ACP** | Interactive client request | **Zero/User** | Slash-command execution | `/vitals`, `/pause`, `/journal` |

### Law of Strict Separation:
- **Senses** ask: *"What does the world look like right now?"* (Descriptive, context-heavy).
- **Stimuli** ask: *"Has a physiological limit been breached?"* (Boolean `triggered: bool`, threshold-checked, zero tokens).

---

## 4. Persona Architecture & Scoped Capabilities

Presence rejects monolithic "all-powerful" agents. System execution is partitioned into specialized **Personas** defined by Markdown YAML cards in `agents/`:

```
               [Human Owner / ACP Client]
                           |
                           v
              +-------------------------+
              |   Arche (Prime Cause)   |  <--- Surveys vitals, originates quests,
              +------------+------------+       aligns with constitution, delegates
                           |
         +-----------------+-----------------+
         |                                   |
         v                                   v
+------------------+               +-------------------+
|  Organcrafter    |               |     Mechanic      |
| (Peripheral Dev) |               | (Triad Architect) |
+------------------+               +-------------------+
         |                                   |
         +-----------------+-----------------+
                           |
                           v
              +-------------------------+
              |  Arbiter (Supreme Judge)|  <--- Fault arbiter, loop breaker,
              +-------------------------+       destructive gatekeeper
```

### 4.1 Persona Matrix
| Persona | Role | Primary Organs | Filesystem Scopes (`allow_write`) | Prohibited Scopes (`exclude_write`) |
| :--- | :--- | :--- | :--- | :--- |
| **`arche`** | Prime cause, intentionality driver, owner interface | `state`, `plan`, `io`, `winsense`, `mood`, `social`, `ask`, `vox` | `agents/**`, `workspace/**`, `memory/**` | `src/**`, `Cargo.*`, `organs/**`, `**/.git/**` |
| **`organcrafter`** | Peripheral artisan, crafts/tests/packages organs | `io`, `plan`, `state`, `channel`, `git` | `organs/**`, `registry/**`, `crates/organs/**`, `crates/presence-organ-sdk/**`, `bucket/**` | `src/**`, `Cargo.toml`, `Cargo.lock` |
| **`mechanic`** | Nervous triad systems architect, kernel maintenance | `io`, `plan`, `state`, `channel`, `git` | `src/**`, `Cargo.toml`, `workspace/**`, `memory/**`, `.config/**`, `docs/**` | (None; holds engine authority) |
| **`arbiter`** | Supreme invariant judge, safety gatekeeper | `state`, `io`, `git`, `mood`, `social`, `ask` | `memory/**`, `workspace/**` | `organs/**`, `src/**` |

### 4.2 Non-Preemption & Phase-Boundary Mailbox
- **Critical Section Law**: An active agent is **NEVER** forcibly interrupted or context-switched mid-phase or mid-tool call.
- **Handoff Protocol**: Incoming alarms in `alarms.jsonl` carrying `target_agent: <persona>` wait until the active agent yields `end_turn`.
- **Crisis Exception**: Fatal hardware failures (battery < 5%, disk = 0 bytes) trigger immediate emergency pause (`Mail::Pause`), checkpointing state to `STATE.md` before suspension.

---

## 5. Core Kernel Subsystems (`Dev/presence/src`)

### 5.1 The Hermeneutic Circle Engine (`phase.rs`, `daemon.rs`)
1. **Observation**: Collects mounted senses, git status, friction mood, and active goals.
2. **Plan**: LLM formulates structured JSON containing thought, situation analysis, and tool call sequence.
3. **Verification / Anticipation**: Validates proposed actions against persona capabilities and budget limits.
4. **Execution (`Vollzug`)**: Runs tool calls, streams UI chunks, monitors exit codes, and detects `goal_done`.

### 5.2 Dynamic Tool Discovery & Disjunctive Dependencies (`tools.rs`)
- **Two-Pass Discovery**:
  - *Pass 1*: Discovers all candidate `organ.yaml` manifests in workspace and system registries.
  - *Pass 2*: Evaluates granular dependencies across all candidates.
- **Disjunctive Dependency Syntax**:
  - System binaries: `any_of: ["sfsu", "scoop", "nix", "guix"]`.
  - Dependent organs: `dependencies.organs: ["winsense || linsense"]` or `any_of: ["winsense", "linsense"]`.
- **Context Injection**:
  - Core constructs `PRESENCE_ORGAN_CONTEXT` JSON:
    ```json
    {
      "satisfied_organs": ["winsense", "io"],
      "satisfied_binaries": ["pwsh", "git"],
      "disabled_features": [],
      "disabled_tools": []
    }
    ```
  - Injected into child processes, enforcing environmental least-privilege.

### 5.3 Process Sandboxing (`sandbox.rs`)
- **Windows**:
  - Wraps process execution in **Win32 Job Objects**.
  - `CREATE_SUSPENDED` -> assign to Job Object -> `ResumeThread`.
  - Enforces `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (prevents rogue orphan processes) and hard memory commit limits.
- **Linux**:
  - Scaffolding for **Bubblewrap** (`bwrap`) with unshared user/PID/mount namespaces and read-only filesystem mounts.

### 5.4 Context Budgeter (`budgeter.rs`)
- Strict token accounting for local and cloud models.
- Priority-based context packing: Pinned excerpts > Core prompt > Goals > Senses > History.
- Pre-flight compaction: If estimated tokens exceed window limits, triggers single-turn episodic summary before making tool calls.

---

## 6. The Organs Catalog & Workspace (`Dev/presence-organs`)

### 6.1 `presence-organ-sdk`
- The standard library for compiling pure-Rust organs.
- **Macros**: `organ_ok!(payload)` and `organ_err!(message)` for standard JSON output.
- **CLI Parsing**: `OrganArgs::from_env()` parsing `--tool <name>`, `--sense <name>`, `--stimulus <name>`, `--reflex <name>`, `--action <name>`.
- **Runtime Resolution**: `resolve_any_binary(&[&str])` for zero-crash fallback selection.
- **Capabilities**: `OrganContext::current()` to inspect satisfied dependencies without disk I/O.

### 6.2 Catalog of Tested Organs
1. **`organ-shell`**: Sandboxed command execution (`exec_command`, `spawn_background`, `poll_task`, `kill_task`). Dynamic shell fallback (`pwsh -> powershell -> cmd` on Windows; `bash -> zsh -> sh` on Linux).
2. **`organ-packager`**: Cross-platform dependency resolution (`scoop`/`sfsu` and `nix`/`guix`). Tool `packager_resolve` analyzes missing organ requirements and auto-installs.
3. **`organ-vitals`**: Native hardware telemetry extractor (`system_vitals`, `vitals_summary`). Win32 FFI (`GetSystemTimes`, `GlobalMemoryStatusEx`, `GetSystemPowerStatus`). Stimuli: `battery_critical`, `cpu_throttle`.
4. **`organ-ask`**: Human interaction gating (`ask_question`, `ask_confirm`, `ask_text`). Topmost Win32 modal dialog fallback; audit logs to `logs/ask/inquiries.jsonl`.
5. **`organ-channel`**: Asynchronous inter-agent and owner messaging (`channel_send`, `channel_poll`, `channel_history`). Decoupled from ACP.
6. **`organ-io`**: Path-sanitized disk I/O (`io_read_file`, `io_write_file`, `io_list_files`).
7. **`organ-memory`**: Two-tier memory (`memory_store`, `memory_recall`, `memory_forget`). Episodic ring buffer + semantic storage with power-law recency decay ($S = e^{-\lambda \Delta t}$).
8. **`organ-mood`**: Affective state calculation (`current_mood`, `record_friction`). Translates mechanical friction strikes into posture (steady, heavy, strained).
9. **`organ-social`**: Interlocutor relationship tracking (`note_habit`, `tune_register`, `interlocutor_profile`).
10. **`organ-state`**: Agent state checkpointing (`state_snapshot`, `state_restore`, `state_inspect`).
11. **`organ-issues`**: Alpha/Beta telemetry and bug reporting (`report_issue`, `log_friction`, `list_issues`).
12. **`organ-manual`**: On-demand documentation reader (`manual_list`, `manual_read`, `manual_search`).
13. **`organ-winsense`**: Win32 sensory receptor for foreground windows, idle time, and power state.
14. **`organ-vox`**: Pure-Rust audio capture and TTS playback.

### 6.3 Standalone Utilities
- **`presence-acp`**: Standalone Agent Client Protocol gateway binary. Communicates over stdio JSON-RPC 2.0. Supports native Zed profiles (grey thinking chunks, available slash commands autocomplete, click-to-file cards).
- **`presence-dashboard`**: Native desktop UI built with `eframe`/`egui`. Displays live vitals gauges, active stimuli, organ registry status, and recent alarms.

---

## 7. Forensic State: The Absolute Truth Matrix

| Subsystem / Feature | Architectural Intent | Real State in Code | Status | Exact Source Location |
| :--- | :--- | :--- | :--- | :--- |
| **StemBus Stimulus Polling** | Autonomous vegetative stimuli poller | **Implemented & Integrated** in `stem.exe` | **VERIFIED REAL** | `src/stembus.rs`, `src/bin/stem.rs:76-140` |
| **Stem Heartbeat Modulation** | Pulse slows to 30s on idle, 60s on lock | **Implemented & Active** in `stem.exe` | **VERIFIED REAL** | `src/bin/stem.rs:102-132` |
| **Stem Brainless Mode** | Offline vegetative mode on network drop | **Implemented & Active** in `stem.exe` | **VERIFIED REAL** | `src/bin/stem.rs:113-122` |
| **Cord Reflex Arc** | Sub-ms involuntary interception before LLM | **Implemented & Evaluated** in `stem.exe` | **VERIFIED REAL** | `src/cord.rs`, `src/bin/stem.rs:141-185` |
| **Tag System in Stem (vs Timer)** | Event/entropy-driven reactive wake tags | **NOT IMPLEMENTED** (Concept in research only) | **MISSING** | Only in `RESEARCH_...md`; `stem.rs:92` is `thread::sleep` |
| **Disjunctive Dependencies** | `any_of: [...]` & `||` across tools/organs | **Implemented & Tested** in `tools.rs` | **VERIFIED REAL** | `src/tools.rs:137-330` (commit `4e0e69e`) |
| **Scoped Context Injection** | `PRESENCE_ORGAN_CONTEXT` env passed | **Implemented** in `tools.rs` & `sdk` | **VERIFIED REAL** | `src/tools.rs:1175-1185`, `sdk/src/lib.rs` |
| **Telemetry Env Injection** | `PRESENCE_USER_IDLE_SECS` injected | **Coupled Bug** (hardcoded to `winsense`) | **ARCHITECTURAL BUG**| `src/tools.rs:1188` calls Win32 `winsense` directly |
| **Native Vitals Organ** | Hardware telemetry extracted from core | **Implemented & Packaged** in catalog | **VERIFIED REAL** | `crates/organs/vitals` (commit `b7df363`) |
| **Linux Sensor (`linsense`)** | Cross-platform equivalent of `winsense` | **NOT IMPLEMENTED** (Only schema references) | **MISSING** | No crate or source exists |
| **Inter-Organ 5-Entity RPC** | Organs calling peer tools/stimuli/reflexes | **REJECTED HALLUCINATION** | **PURGED** | Hallucinated in `HANDOFF_STEMBUS...md`; refuted |
| **Decentralized GPU Compute** | Akash Network + vLLM Qwen2.5-Coder HTTP | **NOT STARTED** (Roadmap Phase 2C / Section 12) | **DEFERRED** | Still uses `proxy.mjs` + `agentrouter.org` |

---

## 8. Master Plans & Future Roadmap

### 8.1 Decentralized GPU Compute (Akash Network) & Qwen2.5-Coder
- **Problem**: Current setup relies on centralized `agentrouter.org` and a Node.js TLS-fingerprint proxy (`proxy.mjs`).
- **Target Architecture**:
  - Deploy **vLLM** container hosting `Qwen/Qwen2.5-Coder-32B-Instruct` (AWQ/FP8) on an Akash Network GPU node (1x RTX 4090 24GB or A6000 48GB).
  - Replace `proxy.mjs` with a high-performance, native Rust HTTP client directly calling `/v1/chat/completions`.
  - Configure native Hermes tool calling parser (`--tool-call-parser hermes`, `--enable-auto-tool-choice`).

### 8.2 True Event-Driven Tag System in Stem
- **Problem**: Current `stem.rs` runs a monotonic timer loop (`sleep(5s)` or `sleep(30s)`), polling every stimulus by time elapsed.
- **Target Architecture**:
  - Attach semantic domain tags (`presence:user`, `power:battery`, `git:drift`, `fs:write`) to `ActiveStimulus`.
  - Introduce **Entropy Differential Filtering ($\Delta > 0$)**: a stimulus only emits an event if its measured state metric has changed or crossed a critical threshold.
  - Stem transitions from blind clock ticks to an interrupt-driven autonomic core.

### 8.3 Cross-Platform Parity: `organ-linsense`
- Create `crates/organs/linsense` for Linux systems:
  - Idle detection via `libXss` / `org.freedesktop.ScreenSaver` / `systemd-logind`.
  - Hardware power and battery metrics via `/sys/class/power_supply/`.
  - Window focus detection via X11 (`_NET_ACTIVE_WINDOW`) and Wayland protocols.

### 8.4 Telemetry Decoupling in Core
- Refactor [`src/tools.rs:1188`](file:///C:/Users/hrkcz001/Dev/presence/src/tools.rs#L1188):
  - Strip direct call to `crate::winsense::get_user_idle_seconds()`.
  - Query sensory capability dynamically from registered sensors or omit if unmounted.

---

## 9. File & Schema Reference Guide

### 9.1 `organ.yaml` Manifest Schema
```yaml
name: string                   # Organ unique identifier
version: semver               # Compatibility version
type: "cli" | "typescript"    # Execution runtime
entrypoint: string            # Path to binary or .ts file
manual: string                # Relative path to MANUAL.md
dependencies:
  system:                     # Host requirements
    - "git"
    - any_of: ["sfsu", "scoop", "nix", "guix"]
  organs:                     # Presence organ requirements
    - any_of: ["winsense", "linsense"]
tools:
  - name: string
    description: string
    parameters: json_schema
    requires:
      binaries: [string]
senses:
  - name: string
    description: string
    cadence_secs: u64
stimuli:
  - name: string
    description: string
    cadence_secs: u64
    target_agent: string
    action: "alert" | "modulate_pulse" | "brainless_mode"
```

### 9.2 Core Runtime Paths (`memory/`)
- `memory/alarms.jsonl`: Queue of pending wake invitations for Cortex (`{id, fire_at, reason, origin, target_agent}`).
- `memory/reflexes.json`: Active Cord reflex arc rules (`{id, trigger_stimulus, action, command, target_agent}`).
- `memory/stimuli.json`: User overrides and snooze configurations for stimuli.
- `memory/heartbeat.log`: Append-only audit log of Stem decisions, pulse modulations, and reflex executions.
- `memory/quests.jsonl`: Prioritized goal stack for the Hermeneutic Circle.
- `logs/friction.jsonl`: Mechanical failure strikes and anomaly telemetry.
- `logs/ask/inquiries.jsonl`: Audit journal of all user confirmations and questions.
