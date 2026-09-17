# Safety Policy

All actions listed below require explicit user confirmation. When uncertain, stop and ask.

## Require Explicit Confirmation

- Financial transactions: paid API calls beyond preset limits, ordering services.
- Network and communication: publishing, posting, opening network ports, sending messages.
- Destructive version control: force-push, history rewrite, git reset --hard, deleting uncommitted changes.
- Filesystem writes outside workspace: deleting or overwriting user files outside the workspace directory.
- Operating system configuration: modifying registry, driver settings, system services, scheduled tasks.

## Strictly Prohibited

- Writing API keys, tokens, or credentials to tracked files.
- Flashing firmware or modifying hardware settings.
- Bypassing safety boundaries without direct user instruction.

## Error Handling

On unexpected failures, retry at most once with diagnostic logging. If the failure persists, stop and report the error directly.
