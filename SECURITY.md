# Security

## Reporting a vulnerability

Please report it privately: on GitHub, **Security → Report a vulnerability** in this repository. Do not open a public
issue. You should get an answer within a week.

Fixes go into the latest release only.

## What Klat does and does not do

- Everything runs on your machine. No telemetry, no network access beyond `127.0.0.1`.
- The app listens on `127.0.0.1` only, on a random port, behind a fresh 256-bit token per run. It rejects requests
  from web pages (any `Origin` header), bodies over 64 KB and unknown event names.
- The hook program sends five fields per event (event name, session id, project folder name, tool name, agent). It
  never sends prompts, commands, tool input or output, or transcripts, and it never approves or denies anything.
- Installing hooks edits `~/.claude/settings.json` and `~/.codex/hooks.json` after saving a backup, and touches only
  Klat's own entries.

Known limits:

- Releases are not code-signed yet. Check each download against `SHA256SUMS.txt` from the release.
- The Windows installer is per-user (`%LOCALAPPDATA%\Klat`), so the hook program sits in a folder your user account
  can write to, like any per-user app.
