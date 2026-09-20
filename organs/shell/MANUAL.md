# Manual: shell
*Version 0.3.0 — Bounded Shell Execution and Task Manager Organ for Presence*

## Overview
The `shell` organ isolates process execution and operating system command invocation from the Presence Triad core. It provides synchronous bounded command execution (with hard timeouts and multi-byte safe output caps) as well as background process lifecycle management.

## Environment & Dependencies
- **Platforms**: Windows (`x86_64`), Linux (`x86_64`, `aarch64`), macOS (`x86_64`, `aarch64`).
- **Shell Runtimes**:
  - Windows: `pwsh.exe` (PowerShell 7+) with automatic fallback to `cmd.exe`.
  - Unix/macOS: `sh` / `bash`.
- **Permissions**: Process execution rights in current user context; write access to `$PRESENCE_WORKSPACE/logs/tasks/`.

## Tools Specification

### `exec_command`
Executes a command synchronously within the specified working directory, enforcing execution deadlines and output size caps.
- **Parameters**:
  - `command` (`string`, required): Shell command line to execute.
  - `cwd` (`string`, optional): Working directory path. Defaults to workspace root.
  - `timeout_secs` (`number`, optional, default: `30`): Hard timeout in seconds.
- **Return Shape**:
  ```json
  {
    "status": "ok",
    "data": {
      "command": "cargo check",
      "exit_code": 0,
      "success": true,
      "stdout": "    Finished `dev` profile",
      "stderr": "",
      "duration_ms": 1420
    }
  }
  ```
- **Gotchas / Edge Cases**:
  - Output is truncated to 32,000 bytes with a `[truncated]` trailer to prevent context blowup.
  - Commands exceeding `timeout_secs` are forcefully killed with SIGKILL / `child.kill()`.

### `spawn_background`
Launches an asynchronous, detached background task, streaming its output to disk.
- **Parameters**:
  - `command` (`string`, required): Long-running command.
  - `task_id` (`string`, optional): Custom unique ID. Generated if omitted.
- **Return Shape**:
  ```json
  {
    "status": "ok",
    "data": {
      "action": "spawned",
      "task_id": "build-job-1",
      "pid": 28410,
      "log_file": "C:/workspace/logs/tasks/build-job-1.log"
    }
  }
  ```

### `poll_task`
Inspects the live execution state and recent output tail of a background job.
- **Parameters**:
  - `task_id` (`string`, required): Target task ID.
- **Return Shape**:
  ```json
  {
    "status": "ok",
    "data": {
      "task_id": "build-job-1",
      "pid": 28410,
      "status": "running",
      "command": "cargo build --release",
      "output_preview": "... compiling crate"
    }
  }
  ```

### `kill_task`
Forcefully stops a running background task process tree.
- **Parameters**:
  - `task_id` (`string`, required): Target task ID.

## Senses & Stimuli

### Stimulus: `command_hung`
- **Cadence**: 10s.
- **Trigger**: Any active task in `logs/tasks/` exceeding maximum permitted runtime.
- **Reflex**: Triggers task termination and alerts Stem/Cortex.

## Failure Modes & Recovery
- **Missing Shell**: On Windows, if `pwsh.exe` is absent, automatically attempts fallback to `cmd.exe`.
- **Pipe Buffer Deadlocks**: Stdout and Stderr are drained concurrently on dedicated threads, preventing OS pipe saturation deadlocks.