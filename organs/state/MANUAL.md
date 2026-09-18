# Manual: state

Presence cognitive state preservation, session pause, and hibernation organ.
Version: 0.3.0

## Overview
`state` manages agent lifecycle, checkpoints, and safe resting states.
When pausing or resting, it serializes persona working cards, memory pointers, and active objectives into snapshot archives.

## Environment & Dependencies
- **Platforms**: Windows, Linux, macOS
- **Runtime / Binary**: Native binary (`organ-state.exe` on Windows, `organ-state` on Linux)
- **Dependencies**: None (native Rust executable)
- **Permissions**:
  - File read/write to `memory/snapshots/`, `memory/state/`, and `agents/*.agent.md`.
  - Timeout: 15s.

## Tools Specification

### `pause_session`
- **Description**: Pause current session, write a pause snapshot, and transition the active agent card into `rest` mode.
- **Parameters**:
  - `reason` (`string`, required): Why the session is pausing.
- **Example**: `pause_session(reason: "user requested break")`

### `snapshot`
- **Description**: Create an explicit labeled state snapshot of the active agent card.
- **Parameters**:
  - `label` (`string`, optional): Checkpoint identifier (default: "manual").
- **Example**: `snapshot(label: "checkpoint_pre_refactor")`

### `restore`
- **Description**: Restore previously created state snapshot to the active agent card.
- **Parameters**:
  - `label` (`string`, optional): Snapshot name or substring match (defaults to most recent).
- **Example**: `restore(label: "checkpoint_pre_refactor")`

## Senses & Stimuli
- **Senses**:
  - `sleep_state` (interval: 60s): Reports whether agent is in active cognition or resting/hibernating mode, with mounted organs and active state keys.
- **Stimuli**:
  - `stale_goal` (cadence: 900s, action: `alert`, target: `arche`): Emitted when active goals in `GOALS.md` remain unupdated for > 12 hours.

## Failure Modes & Recovery
- **Error: `No snapshots found`**:
  - *Cause*: `memory/snapshots/` is empty.
  - *Recovery*: Call `snapshot(label: "init")` to create an initial snapshot.
- **Error: `Agent card not found`**:
  - *Cause*: No `.agent.md` file found in `agents/`.
  - *Recovery*: Check `PRESENCE_AGENT` environment variable or ensure `agents/arche.agent.md` exists.
