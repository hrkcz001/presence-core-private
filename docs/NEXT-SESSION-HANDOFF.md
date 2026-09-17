# NEXT SESSION HANDOFF: Capabilities Engine & Agent Specialization

## Context & Current State
1. **Haven Repository** (`C:\Users\hrkcz001\haven`):
   - Fully cleaned, verified, and decoupled.
   - Holds ONLY the Haven hub and `haven-keeper.agent.md`.
   - Zero presence/dasein code, zero organs, zero untracked relics. Clean git working tree.

2. **Presence Repository** (`C:\Users\hrkcz001\Dev\presence`):
   - Standalone Git repository for the Presence autonomous runtime.
   - Pure Rust engine (`Cargo.toml`, `src/`), modular peripheral organs (`organs/`), seed personas (`agents/`).
   - 89 passing unit/integration tests (`cargo test`).

---

## Answers to Architectural Questions

### 1. How is `toolcrafter` editing tools while default agents cannot?
- **Currently**: It is strictly a **prompt-level role separation** (aspirational/behavioral). Both cards currently have flat `writes: true` and `runs_shell: true`. There is NO runtime restriction blocking `presence` from editing tools or modifying itself.
- **How it MUST work via Scoped Capabilities Engine (Priority 1)**:
  - **Scoped Paths**:
    - Default `presence`: Granted `fs:write` scoped strictly to `["workspace/**", "memory/**"]`. Access to `src/` or `organs/` is forbidden by the Rust runtime.
    - `toolcrafter`: Granted `capability: organ:craft` and `fs:write` scoped to `["organs/**"]`.
    - `self-improver` / `mechanic`: Dedicated persona with `engine:repair` privilege and access to `src/`, requiring explicit escalation or sandbox verification.

### 2. Specialized Personas Needed in Presence:
- **`presence`** (Orchestrator / General Mind): High-level reasoning, planning, task delegation, observation. Cannot edit runtime or organs directly.
- **`toolcrafter`** (Peripheral Artisan): Crafts, modifies, tests, and packages `organs/` and external tool bindings.
- **`mechanic` / `debugger`** (Self-Inspection & Repair): Debugs Rust engine, runs `cargo test`, analyzes panic logs, performs self-repair within bounded sandbox.

### 3. Agent Card Instructions Review:
- Current cards rely heavily on qualitative prose and lack:
  - Explicit capability tokens (`capabilities: [...]` with path scopes).
  - Machine-verifiable preconditions and invariants.
  - Strict input/output schemas for handoffs between agents.

---

## Action Plan for Next Session:

1. **Implement Scoped Capabilities Catalog** (`Dev/presence/.config/capabilities.yaml`):
   - Formal schema for `fs:read`, `fs:write` (scoped to paths), `proc:spawn`, `organ:mount`, `engine:modify`.
2. **Update Agent Cards in Dev/presence**:
   - `presence.agent.md`: Standard user operations, forbidden from touching engine/organs.
   - `toolcrafter.agent.md`: Scoped exclusively to `organs/**` and test commands.
   - Add `mechanic.agent.md`: Persona dedicated to engine debugging and self-repair.
3. **Implement Rust Capability Validation** (`Dev/presence/src/capabilities.rs`):
   - Check path scopes against granted capabilities.
   - Enforce immutable core (block modifications to `src/` unless persona has `engine:modify`).
   - Enforce attenuation on `switch_agent` / creation (Child <= Parent).
