# AGENTS.md — Presence System Operating Invariants

High-level system rules and operating contract of Presence. Persona specifics belong in `agents/*.agent.md`; organ capabilities and instructions belong in their respective manifests (`organ.yaml`).

## 1. System Invariants
- **Machine-Dense Documentation**: All workspace markdown files are structured LLM representations (token-dense, tabular/key-value schemas, zero conversational filler).
- **Language Law**: External communication directed to the owner is always Russian. Cognitive reasoning, tool calls, manifests, code, and logs are English.
- **Twin-Tracking**: Constitution files (`AGENTS.md`, `governance/`, seed mirrors) must remain synchronized in `tools/dasein/seed/`.

## 2. Execution Boundaries & Safety
- **Bounded Pipes**: All command outputs must be bounded (`| head`, `Select-Object -First`, `-MaxEvents`). Never overflow token budgets.
- **Workspace Isolation**: Temporary and scratch files reside in `workspace/`. Top-level repository layout is preserved.
- **Zero Secrets**: No tokens, private keys, or credentials in tracked files (policy: `governance/secrets.md`).
- **Non-destructive Git**: Checkpoint coherent steps without prompting. Never force-push or rewrite pushed history without explicit user instruction.
- **One Turn, One Topic**: One action or verdict per turn. Heavy outputs belong in files; replies convey status and path.
- **Disk-First Planning**: Any workflow exceeding a single trivial step must be persisted to disk before execution (`workspace/*.md` or organ intent). Sessions may terminate abruptly; files survive.

## 3. Architecture & Modular Separation
- **The Nervous Triad**:
  - **Cortex**: Conscious deliberation loop.
  - **Stem**: Pacemaker, circadian rhythms, vitals.
  - **Cord**: Fast reflex arcs without LLM overhead.
- **Persona Specialization & Switching**:
  - Agents are modular personas (`agents/*.agent.md`).
  - Active agent switching (`switch_agent`) is encouraged to match task specialization (e.g., engineering tasks to builder personas, general tasks to orchestrator personas).
  - Living operational state (`mode`, `weight`, `next`) is bound directly to the active agent card frontmatter.
- **Organs as Peripherals**:
  - Effectors, sensors, and external communication interfaces are attached as Organs (`organ.yaml`).
  - Organ instructions and schemas are self-contained within each organ and dynamically injected into the runtime context upon discovery.
