# Presence & Organs Architecture: Master Plan & Roadmap

## 1. Package Manager & Dependencies Blueprint
- **Single-Command Imperative Operations**:
  - All package management across systems must work out of the box via single CLI invocations with zero config file editing.
  - **Windows**: `scoop install <pkg>` (accelerated via `sfsu` as an internal driver).
  - **Linux (Nix)**: `nix-env -iA nixpkgs.<pkg>` or `nix profile install nixpkgs#<pkg>`.
  - **Linux (Guix)**: `guix install <pkg>`.
- **Runtime Dependencies**:
  - `bun`: Required runtime for all TypeScript/JavaScript scripts and lightweight tooling across systems. Declared explicitly in dependency graph and installed if missing.
  - `pwsh`: Required shell environment on Windows (replaces legacy `cmd`/`powershell 5.1`).
  - `sfsu`: Mandatory dependency under Scoop on Windows for sub-millisecond querying/listing.
- **Packager Organ Scope**:
  - Senses (`outdated_organs`) must monitor ONLY Presence organs (registry/git/local), not the entire operating system software suite.
  - General system package management is reserved for on-demand tools (`packager_install`, `packager_search`).

## 2. Standardized `MANUAL.md` Specification
- Every organ must include a strictly parseable `MANUAL.md` in its root folder so it can be read directly by humans, agents (`read_file`), or parsed programmatically by `organ-manual`.
- **Schema**:
  1. `# Manual: <organ_name>` (version, brief)
  2. `## Overview`
  3. `## Environment & Dependencies` (platforms, binary requirements, permissions)
  4. `## Tools Specification` (`tool_name`, parameters, gotchas/edge cases, example calls/responses)
  5. `## Senses & Stimuli` (cadence, payload schema, recommended reflexes)
  6. `## Failure Modes & Recovery`
- `organ.yaml` maintains only a reference link (`manual: MANUAL.md`) and short summary to minimize token load.

## 3. Dedicated `organ-manual`
- Standalone Rust organ providing:
  - `manual_list`: Lists all installed organs and documented capabilities.
  - `manual_read(organ, topic?)`: Retrieves structured manual sections and usage gotchas on demand.
  - `manual_search(query)`: Full-text search across all organ manuals in the workspace.

## 4. Execution & I/O Isolation (`organ-shell`)
- Decouple `run_command` from the Presence Triad core.
- Move shell execution into a dedicated organ with:
  - Strict sandboxing and capability bounding.
  - Dependency on `pwsh` on Windows and `sh`/`bash` on Linux.
  - Explicit execution timeout and output buffering controls.

## 5. Two-Tier Memory Organ (`organ-memory`)
- Replace ad-hoc disk files (`active_agent.txt`, loose logs) with a unified memory organ.
- **Short-Term Working Memory**:
  - Fast in-memory circular buffer for recent actions, tool results, and active session stimuli.
  - Sense: `working_memory_pressure` triggers self-reflection and consolidation into long-term storage.
- **Long-Term Semantic Memory**:
  - Embedded vector database (SQLite with vector extensions or LanceDB).
  - Senses: `associative_recall` automatically surfaces relevant past experiences and debugging insights.
  - Stimuli/Reflex: `memory_entropy` implementing Ebbinghaus decay to prune stale and low-importance memories over time.

## 6. Organ `organ-memory`: Complete Replacement of `memory/` Directory
- The legacy `memory/` folder (with ad-hoc text and loose jsonl files) is deprecated.
- **Architectural Shift**: All memory retrieval, working state, and persistent records are managed exclusively through the `organ-memory` organ.
- **Storage Subsystems**:
  - **Working Context Buffer**: In-memory ring buffer (sliding window) for active step execution.
  - **Semantic Vector Storage**: Local embedded database (SQLite with vector extension / LanceDB). Zero external cloud dependency.
- **Cognitive Senses & Stimuli**:
  - `working_memory_pressure`: Senses memory bloat and initiates consolidation reflection.
  - `associative_recall`: Automatically bubbles up relevant past debugging sessions and architectural insights.
  - `memory_decay`: Implements Ebbinghaus forgetting curve to prune noisy or obsolete data.

## 7. State & Affect: `organ-state` (Mood, Friction, and Posture)
- Replaces raw `mood.md` and `STATE.md` with an autonomous organ.
- **Principle: "Felt, not narrated"**: Mood is not an LLM prose generation; it is a deterministic calculation derived from mechanical signals:
  - Phase cycle count, error strikes, execution friction, power/system status.
- Exposes:
  - Sense: `current_state` (posture: steady, heavy, strained).
  - Stimulus: Triggers Arbiter/Arche intervention when cognitive friction exceeds thresholds.

## 8. Re-evaluation of `monologue` Organ
- **Current Problem**: `monologue.ts` acts as a naive `jsonl` logger (`ponder`, `reflect`). Without intelligent synthesis, it clutters context and acts as a distraction rather than a reasoning enhancer.
- **Refined Concept**:
  - Do NOT expose `ponder` as an active tool that wastes prompt turns.
  - Reframe `monologue` into an internal reasoning stream / chain-of-thought scratchpad, or absorb it directly into `organ-memory` as working scratchpad notes that are condensed during the Hermeneutic Circle reflection phase.

## 9. Clarifications & Invariants (Memory, State, and Pause)

### Invariant 1: `memory/` Fallback Protocol
- Removing ad-hoc file clutter MUST NOT break basic organ discovery or standalone execution.
- If `organ-memory` is active, it coordinates working buffers and vector storage.
- If `organ-memory` is absent or not loaded, organs must gracefully fall back to zero-overhead local file append (`inbox.jsonl`, `outbox.jsonl`) without panicking. Organs remain decoupled and self-contained.

### Invariant 2: Monologue Replaced by Memory Sense
- `monologue` as a standalone organ is deprecated and will not exist.
- Instead, `organ-memory` exposes a dedicated sense: `internal_reasoning_stream` / `working_context_sense`, delivering synthesized reflections to the Triad without turn-wasting tool calls.

### Invariant 3: `organ-state` vs `organ-mood` & Pause Status
- **Namespace Conflict Resolved**:
  - `organ-state` is already the existing organ responsible for agent lifecycle state, snapshotting, and pause/restore (`toolSnapshot`, `toolPause`, `toolRestore`, `senseState`).
  - Affective friction ("mood") belongs to a separate dedicated organ: **`organ-mood`** (or integrated as an affective probe into `organ-state`), keeping `organ-state` focused on lifecycle/pause.
- **Audit of Pause Functionality**:
  - In Core (`daemon.rs` / `main.rs`): `Mail::Pause` handles `/pause` and immediate circle suspension cleanly via ACP events.
  - In `state.ts`: `toolPause` creates a snapshot and marks the card mode as `rest`. Needs a Rust rewrite to remove TypeScript boilerplate, unify with core's `Mail::Pause`, and ensure 100% up-to-date protocol compliance.

## 10. Alpha/Beta Bug & Telemetry Organ: `organ-issues`
- **Purpose**: During testing, equip the agent with tools to capture bugs, UX friction, capability gaps, and tool failure spikes into local logs and GitHub Issues.
- **Stance & Injection**: When mounted, explicitly instructs the agent to log anomalies and friction via `organ-issues` instead of papering over errors.
- **Lifecycle**: Enabled by default in Alpha/Beta (v0.3.x - v0.9.x); transitions to opt-in (`--telemetry` / config flag) in v1.0.0+ stable releases.
- **Tools**: `report_issue`, `log_friction`, `list_issues`. Sense: `recent_friction`.
- **Roadmap Execution**: Tracked as Phase 2 in `workspace/projects/presence/ROADMAP-5-PHASES.md`.

## 11. Dedicated ACP Gateway (`presence-acp`) & Clean `organ-channel`
- **Decoupling Mandate**:
  - `organ-channel` must be strictly an organ for agent communication (tools: `send_reply`, `send_status`, sense: `incoming_inbox`).
  - All embedded ACP protocol logic (`run_acp_bridge()`) is stripped out of `organ-channel`.
- **Standalone `presence-acp` Binary**:
  - Acts as the universal JSON-RPC 2.0 stdio bridge between external editor clients and the Presence runtime.
  - **Multi-Client Architecture**:
    - **Default/Standard ACP Protocol**: Zero-assumption JSON-RPC 2.0 implementation following the Agent Client Protocol spec.
    - **Zed Profile (`--profile zed` / auto-detected)**:
      - Emits `sessionUpdate: "agent_thought_chunk"` for grey thinking UI.
      - Emits `sessionUpdate: "available_commands_update"` for native slash-commands menu.
      - Emits `sessionUpdate: "plan"` for dynamic checklist UI.
      - Tool cards with location navigation (`locations: [{"path": ...}]`).
    - **Extensible Profiles**:
      - `vscode`: Protocol adapter for VSCode client integrations (e.g. Cline/Roo-Code ACP protocol).
      - `cli`: Clean human-readable streaming terminal interface.

## 12. Decentralized GPU Compute (Akash Network) & Qwen2.5-Coder Model Architecture
- **Infrastructure Shift**:
  - Replace centralized `agentrouter.org` and its required Node.js TLS-fingerprint shim (`proxy.mjs`) with self-hosted instances on **Akash Network**.
  - Deploy **vLLM** container hosting **`Qwen/Qwen2.5-Coder-32B-Instruct`** (AWQ/FP8 quantization on 1x RTX 4090 24GB or A6000 48GB).
- **Direct Rust HTTP Engine**:
  - Presence Core communicates directly with vLLM's standard OpenAI-compatible `/v1/chat/completions` endpoint via high-performance native Rust HTTP calls (removing Node.js dependency).
- **Core Triad Adaptations**:
  - **Context Budgeting (`budgeter.rs`)**: Tune target window to 32k - 64k tokens with aggressive episodic pruning.
  - **Tool Calling**: Native Hermes/Qwen function calling parser (`--tool-call-parser hermes`, `--enable-auto-tool-choice`).
  - **Structured Plan Guidance**: Leverage vLLM guided JSON decoding for 100% deterministic schema adherence during Hermeneutic Circle phases (`plan`, `situation`).
  - **Self-Evolution**: Fine-tune system prompt and prompt constitution for rapid compiler diagnostic resolution (`cargo check`, `tsc`) and organ creation.

## 13. Deferred Evolution: Emergent Attentional Focus in the Hermeneutic Circle
- **Concept & Intent**:
  - Transform the Hermeneutic Circle from a static linear phase pipeline into an emergent cognitive filter.
  - As the circle transitions through phases (Observation -> Orientation -> Interpretation -> Plan -> Execution -> Reflection), each phase dynamically and emergently selects its focal plane, pruning irrelevant noise and narrowing attentional bandwidth.
- **Architectural Placement Reflections**:
  1. **Option A: Native to the Hermeneutic Circle Engine (`phase.rs` / `daemon.rs`)**:
     - *Advantage*: Zero overhead, direct deterministic token budgeting and phase output shaping.
     - *Risk*: Blurs the pure "stateless, toolless" invariant of the kernel if heuristics become overly complex.
  2. **Option B: Nervous Triad Layer (Cord / Stem Attentional Gating)**:
     - *Advantage*: Cord already manages reflexes and fast inhibition; Stem already manages event routing. Treating attention as a biological gating mechanism keeps the Hermeneutic Circle pure while enabling adaptive context focus.
  3. **Option C: Cognitive Organ (`organ-attention`)**:
     - *Advantage*: Full modularity; agent personas can choose whether they possess sharp focused attention (e.g. Arche) or broad diffuse awareness (e.g. Arbiter).
     - *Tradeoff*: Requires an extra tool/sense round unless integrated into pre-turn prompt assembly.
- **Decision**: Deferred for evaluation after decentralized compute and core stabilization.

## 14. Primary Vector: Tri-Modal Ontological Cycle (Jev System-1 + Deliberative LLM + Psychosomatic Feedback)
- **Status: HIGHEST PRIORITY / ARCHITECTURAL NORTH STAR**.
- **Context & Motivation**:
  - Replaces previous ad-hoc multi-phase ReAct iterations with an authentic Heideggerian ontological cycle (*Dasein: Geworfenheit -> Verfallen -> Entwurf*).
  - Eliminates excessive token burn and multi-second latency by deploying **Jev System-1** (non-autoregressive, calibrated probabilities via RLCD) for 90% of routine sensory gating and tool selection at sub-50ms latency.
  - Generative System-2 LLMs (DeepSeek / Astra) are strictly quarantined to semantic argument and code synthesis (*Entwurf*).
- **Core Architecture & Phase Flow**:
  1. **Autonomic Budget & Organ Payload Limits**:
     - Every organ declares hard caps in `organ.yaml`: `sense_output_max_bytes`, `stimulus_payload_max_bytes`, `tool_stdout_max_bytes`.
     - Vegetative Stem (`stem.rs`) enforces somatic pre-gating: sum of senses must not exceed Jev budget. Under physiological stress (battery drain, friction spike), somatic tunnel vision drops low-tier senses before Jev runs.
  2. **Modus 1: Geworfenheit (Заброшенность / Фактичность)**:
     - Senses surviving somatic gating enter Jev.
     - Jev scores sense priorities (`priority_<id>: score 1..100`) and evaluates dynamic focus threshold `theta_focus`.
     - Output: Attentive focus set $S_{attentive} = \{ s \mid priority(s) \ge \theta_{focus} \}$.
  3. **Modus 2: Verfallen (Падение / Бытие-при-сущем / Поглощенность делами)**:
     - Jev takes $S_{attentive}$ and available ready-to-hand tools from `organs/`.
     - Evaluates `selected_tool: choice`, `needs_projection: noul`, and `projected_senses: choice[]`.
     - **Psychosomatic Feedback (*Befindlichkeit* / Настроенность)**: Downward projection of somatic keywords into `stem.rs` to modulate heartbeat cadence (5s -> 1s in alarm/hyper-focus) and retune somatic pre-filters for future cycles.
     - If `needs_projection == false`: execute tool directly via binary invocation (0 LLM tokens, <1ms).
  4. **Modus 3: Entwurf (Набрасывание / Бытие-вперед-себя / Проектирование возможностей)**:
     - If `needs_projection == true`, absorbed routine is broken. Dasein projects itself understandingly onto its future possibilities.
     - Generative LLM (DeepSeek / Astra) receives `(selected_tool_schema, projected_senses, goal)` to synthesize the required payload (code, diffs, shell arguments).
     - Static spinal reflexes in `memory/reflexes.json` remain pre-compiled safety invariants evaluated in the vegetative stem (dynamic reflex crystallization rejected as premature complexity).
- **Deliverables**:
  - Native Rust HTTP client for Jev API (`src/jev.rs`) with question batching.
  - Refactoring `src/phase.rs` and `src/daemon.rs` into the Three Moduses.
  - Adding `limits` schema to `presence-organ-sdk` and manifests in `Dev/presence-organs`.
