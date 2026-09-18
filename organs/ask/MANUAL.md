# Manual: ask

Interactive user elicitation, confirmation gating, and structured interrogation organ.
Version: 0.3.0

## Overview
`organ-ask` equips the agent with direct, structured, interactive questioning capabilities.
When an agent encounters ambiguity, underspecified instructions, multiple architectural options, or hazardous operations, `organ-ask` enables direct clarification without guessing or hallucinating user preferences.

## Environment & Dependencies
- **Platforms**: Windows (x86_64), Linux (x86_64, aarch64)
- **Runtime**: Native pure-Rust compiled binary (`organ-ask.exe` / `organ-ask`).
- **Dependencies**: None. Zero external network or package manager requirements.
- **Storage**: `logs/ask/` or `memory/ask/` for session question audit trails.

## Tools Specification

### `ask_question`
- **Description**: Presents an interactive multiple-choice inquiry to the user with optional multi-select and write-in support.
- **Parameters**:
  - `question` (`string`, required): The interrogation text.
  - `options` (`array<string>`, required): List of candidate responses (at least 2).
  - `is_multi_select` (`boolean`, optional, default `false`): Allow selecting multiple options.
- **Output Schema**:
  ```json
  {
    "status": "ok",
    "question": "Which backend should we configure for package installation?",
    "selected": ["scoop"],
    "is_write_in": false
  }
  ```

### `ask_confirm`
- **Description**: High-stakes confirmation gate (Yes/No) for operations with side effects (file deletion, git reset, engine rebuild).
- **Parameters**:
  - `prompt` (`string`, required): Description of the operation needing approval.
- **Output Schema**:
  ```json
  {
    "status": "ok",
    "confirmed": true,
    "prompt": "Proceed with wiping scratch logs?"
  }
  ```

### `ask_text`
- **Description**: Open-ended single-line or multi-line inquiry to solicit user clarification.
- **Parameters**:
  - `prompt` (`string`, required): The question text.
  - `placeholder` (`string`, optional): Suggested input placeholder.
- **Output Schema**:
  ```json
  {
    "status": "ok",
    "response": "Use port 8080"
  }
  ```

## Senses Specification

### `pending_questions`
- **Description**: Senses whether there are pending, unresolved inquiries queued in the session.
- **Format**: Grounding markdown block.

## Slash Commands
- `/ask <prompt>`: Quickly solicit user clarification or open an interactive prompt.
