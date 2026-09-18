# Manual: io

Native pure-Rust physical input/output and filesystem manipulation organ for Presence.
Version: 0.3.0

## Overview
`organ-io` provides low-level file reading, writing, and directory listing directly inside the target workspace.
It compiles into a zero-dependency native Rust binary (`organ-io.exe` / `organ-io`), replacing the legacy Node/Bun TypeScript prototype.

## Tools Specification

### `read_file`
- **Parameters**: `path` (string, required).
- **Description**: Reads UTF-8 file content, returning content, lines, and bytes.

### `write_file`
- **Parameters**: `path` (string, required), `content` (string, required).
- **Description**: Creates necessary parent directories and writes the content.

### `list_files`
- **Parameters**: `path` (string, default ".").
- **Description**: Lists files and directories with size and kind flags.

## Senses
- `fs_changes`: Identifies files modified within the last 5 minutes.

## Slash Commands
- `/cat <path>`: View file text.
- `/ls <path>`: List directory contents.
