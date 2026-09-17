# Presence Roadmap & Priority 1: Scoped Capabilities Engine

## Target Architecture

Presence is an autonomous conscious agent runtime independent of any single host environment.
The primary development target is the Scoped Capabilities Engine to replace flat flags with formal, OS-enforced permissions.

### Priority 1: Scoped Capabilities Engine

1. Formal Capability Catalog (.config/capabilities.yaml):
   - Strictly English schemas.
   - Resource models: filesystem, process, network, audio.
   - Access masks: read, write, create, delete, spawn, connect.
   - Constraint parameters: paths (glob-based with traversal checks), allow_hosts, max_file_size, timeout.

2. Organ Manifest Protocol (organs/*/organ.yaml):
   - capabilities.essential: Minimum required capabilities for organ to mount.
   - capabilities.optional: Extra capabilities that unlock specific tools/senses.
   - unlocks.tools: List of tool names gated by specific capabilities.
   - Dynamic injection of active permissions into organ execution environment (PRESENCE_GRANTED_CAPABILITIES).

3. Core Engine Validation (src/capabilities.rs):
   - Filter defs() dynamically: tools whose optional capabilities are missing are excluded from the LLM prompt.
   - Protected Core Files: Even with fs:write, agents cannot alter runtime engine (src/), core binaries, or safety registries.
   - Child Attenuation: Agents can only spawn or switch to subagents whose permissions are a strict subset of their own (child <= parent).

4. OS-Level Isolation:
   - Windows: AppContainer / Restricted Tokens / Job Objects preventing child process escapes and unapproved filesystem access.
   - Linux/WSL: Landlock LSM and unshare network namespaces.
