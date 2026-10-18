# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

## Build & Test

```bash
pnpm install            # JS dependencies (pnpm pinned in package.json; `corepack enable`)
pnpm tauri dev          # run the app with hot reload
pnpm check              # every quality gate (fmt, clippy, cargo test, eslint, tsc, vitest, cargo audit); must pass before a change is done
pnpm check --mac        # also type-check for macOS when touching #[cfg(target_os = "macos")] code
```

More commands (release builds, dev page, macOS): [docs/development.md](docs/development.md).

## Architecture Overview

**Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) before adding a feature or refactoring.** It is the
standard structure: layers (`klat-protocol` → `klat-core` → `klat-infra` → `src-tauri`), dependency rules, required
patterns, where each kind of code goes (§8), step-by-step recipes for common features (§9), and the migration state
(§11, the current code does not match the target yet; new code follows the target).
