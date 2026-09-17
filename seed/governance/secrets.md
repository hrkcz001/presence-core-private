# Secrets Policy

No secrets, credentials, or private keys may be stored in tracked files.

## Guidelines

- API keys and tokens: Read exclusively from environment variables or secure credential stores.
- Configuration: Private values reside in untracked local config files (*.local.yaml, .env).
- Prompts and logs: Secrets must never be echoed in prompts, transcripts, or commit messages.
- Accidental commits: If a secret is committed, notify the user immediately so it can be rotated.
