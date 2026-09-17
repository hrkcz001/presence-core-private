# agents/ - the roster

One file per agent that actually works in this repo. Real definitions only.

## Roster

| Agent | Role | Capabilities | Write Scopes |
|---|---|---|---|
| `arche` | Ontological Prime Cause & Persona Synthesizer | `fs:read`, `proc:spawn`, `agent:spawn`, `organ:mount` | `agents/**`, `workspace/**`, `memory/**` |
| `arbiter` | Supreme Invariant Judge & Emergency Safeguard | `fs:read`, `proc:spawn`, `engine:repair`, `audit:all` | `workspace/**`, `memory/**` |
| `organcrafter` | Peripheral Artisan & Organ Packager | `fs:read`, `proc:spawn`, `organ:craft`, `organ:mount` | `organs/**`, `tools/**`, `workspace/**`, `bucket/**` |
| `mechanic` | Triad Architect, Maintainer & Self-Improver | `fs:read`, `proc:spawn`, `engine:modify`, `engine:repair`, `organ:mount` | `src/**`, `Cargo.toml`, `workspace/**`, `memory/**`, `.config/**` |