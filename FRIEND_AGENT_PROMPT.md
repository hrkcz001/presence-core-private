You are acting as a Systems Engineer getting the `Presence` runtime up and running.
You have two sister repositories cloned locally:
1. `presence-core-private` (Core Triad: Cortex, Stem, Cord in Rust)
2. `presence-organs-private` (Peripheral Organs catalog & SDK)

Your goal is to inspect the codebase, build both repositories, wire up the missing low-hanging connections, and launch the runtime with our LLM proxy without undergoing any massive architectural rewrites.

---

### Phase 1: Build the Organs and Core

1. Build all organs in `presence-organs-private`:
   cd presence-organs-private
   cargo build --release

2. Copy compiled organ binaries (`organ-shell`, `organ-vitals`, `organ-ask`, `organ-packager`) into the `organs/` directory inside `presence-core-private/`:
   cp target/release/organ-* ../presence-core-private/organs/

3. Build the Core runtime in `presence-core-private`:
   cd ../presence-core-private
   cargo build --release

   This produces `target/release/presence` (Cortex / Agent) and `target/release/stem` (Vegetative Stem / Autonomic heartbeat).

---

### Phase 2: Apply 3 Minor Tinkering Fixes (Do NOT Rewrite Architecture)

Inspect the following files and make these pragmatic adjustments:

1. **Decouple `src/tools.rs:1188` (`PRESENCE_USER_IDLE_SECS`)**:
   - Locate the hardcoded line: `let idle_secs = crate::winsense::get_user_idle_seconds() as u64;`.
   - If running on non-Windows (or without `winsense`), wrap it with `#[cfg(windows)]` or fallback safely to `0` / reading an environment variable so the build doesn't panic or fail on Linux.

2. **Verify Reflex Interception (`memory/reflexes.json`)**:
   - Verify `stem.rs` and `src/cord.rs`. Stem polls background stimuli via `StemBus` and passes events to `cord.evaluate()`.
   - Make sure `memory/reflexes.json` contains a valid JSON array of rules so benign alerts don't spam `alarms.jsonl`.

3. **Verify Organ Discovery**:
   - Ensure the YAML manifests in `organs/` match the binary filenames copied from Phase 1. Run `cargo test` to ensure all disjunctive dependency checks pass.

---

### Phase 3: Setup LLM Proxy (AgentRouter Bridge)

A streaming reverse proxy script `ar-proxy.mjs` is included in the root of `presence-core-private`. It masks outbound requests with `codex_cli_rs` headers to satisfy AgentRouter's client whitelist and stream reasoning tokens without buffering.

1. Start the proxy in a background terminal:
   node ar-proxy.mjs
   (Listens on `http://127.0.0.1:4000/v1` and bridges to `https://agentrouter.org`).

2. Configure `.config/presence.yaml`:
   models:
     - name: gpt-4o # or deepseek/deepseek-chat, or your configured model
       context_limit: 65536
       max_output_tokens: 8192
       capabilities:
         tools: true
         parallel_tool_calls: true
       endpoints:
         - base_url: http://127.0.0.1:4000/v1
           api_keys:
             - "YOUR_AGENTROUTER_API_KEY"

---

### Phase 4: Run the Runtime

Launch both layers in separate terminals:
- Terminal 1 (Vegetative Stem — 0 tokens, background sensor loop & reflexes):
  ./target/release/stem

- Terminal 2 (Conscious Cortex — Hermeneutic Circle agent):
  ./target/release/presence

Report any compilation errors or missing bindings back to the user.
