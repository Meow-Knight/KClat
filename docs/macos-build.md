# Building and running Klat on macOS (from source)

For developers. This runs Klat straight from the source tree with `pnpm tauri dev`, and builds the `.dmg` for
other people ([Building the .dmg](#building-the-dmg)). People who only install the `.dmg` follow
[macos-install.md](macos-install.md).

> **Status (2026-10-13):** the macOS code has been type-checked from Windows (`pnpm check:mac`) but has **not
> run on a real Mac yet**. Your first run is the first real test. Please note everything that goes wrong
> (see [Reporting back](#reporting-back)).

## What you need

| | How to get it |
|---|---|
| macOS 11 (Big Sur) or later, Apple Silicon or Intel | |
| Xcode Command Line Tools | `xcode-select --install` |
| Rust (stable) | https://rustup.rs, then `rustup default stable` |
| Node.js 20.19 or later | https://nodejs.org or `brew install node@20` |
| pnpm 10 | `corepack enable` (uses the version pinned in `package.json`; pnpm 8 and 9 have known install-time CVEs) |
| Claude Code | To see the cat react to real sessions |

## Run it

```bash
git clone <this repository> klat && cd klat
pnpm install
cargo build -p klat-hook          # the hook program, next to the dev app in target/debug/
pnpm tauri dev                    # builds and starts Klat; the first build takes a few minutes
```

What you should see:

1. A cat icon in the **menu bar** (top right). There is no Dock icon: Klat is a menu bar app.
2. A sleeping cat in the bottom-right corner of the main screen, on a transparent background. Clicks on the
   empty area around it go through to the window below.

The menu bar menu has: Ready, Pause notifications, Dismiss all, **Settings…**, Quit.

## First-time setup

### 1. Let Klat see the Space key (to drag the cat)

Dragging the cat (hold Space, press the mouse on the cat, move, release Space) needs the **Accessibility**
permission.

1. Menu bar icon → **Settings…** → **General**. Under *Moving the cat* the permission shows **Allow…**.
2. Click it. macOS shows its prompt and opens **System Settings → Privacy & Security → Accessibility**.
3. Turn on the app that **started** Klat. With `pnpm tauri dev` that is your terminal (Terminal, iTerm,
   VS Code…), not "Klat": macOS gives the permission to the app that launched the process.
4. Within about 2 seconds the row in Settings changes to **Allowed** and dragging works. No restart needed.

Everything else works without this permission.

### 2. Install the Claude Code hooks

Settings → **Hooks** → **Install**. This adds Klat's entries to `~/.claude/settings.json` (after saving a
backup) and points them at `target/debug/klat-hook`. Restart open Claude Code sessions.

If you later install Klat from a `.dmg`, open Settings → Hooks again and click **Repair** so the hooks point
at the installed copy.

### 3. Try it

With the app running, in another terminal:

```bash
scripts/send-test-event.sh PermissionRequest --tool Bash   # the cat wakes up with its aura and yells
scripts/send-test-event.sh PostToolUse --tool Bash         # back to sleep
```

Or use Settings → **Advanced** → **Wake test**.

## Where things are

| What | Where |
|---|---|
| Settings, `runtime.json`, log (`klat.log`) | `~/Library/Application Support/Klat/` |
| Claude Code hooks | `~/.claude/settings.json` (backups next to it: `settings.json.bak.<time>`) |
| Hook program (dev) | `target/debug/klat-hook` |

## Building the .dmg

One `.dmg` for Apple Silicon and Intel ("universal"). It has to be built on a Mac.

No Mac? GitHub builds it: [`.github/workflows/release.yml`](../.github/workflows/release.yml) runs the
steps below on a macOS runner (and `pnpm release` on a Windows one). **Actions → Release → Run workflow** leaves the `.dmg` and `SHA256SUMS.txt`
as the run's artifact (`klat-macos-universal`). Pushing a tag `v<version>` (the version in
`src-tauri/tauri.conf.json`, e.g. `v0.1.0`) also attaches them, with the Windows installer, to a draft release, which you then publish.

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin   # once; the script also does it
pnpm release:mac
```

`pnpm release:mac` runs two steps:

1. `node scripts/stage-sidecar.mjs --target universal-apple-darwin` builds `klat-hook` for both chips and joins
   them with `lipo` into `src-tauri/binaries/klat-hook-universal-apple-darwin` (add `--dry-run` to only print
   the steps).
2. `tauri build --target universal-apple-darwin --config src-tauri/tauri.release.conf.json` builds the app for
   both chips and bundles it. `src-tauri/tauri.macos.conf.json` adds the macOS settings: `app` and `dmg`
   targets, macOS 11.0 minimum, ad-hoc signing (`signingIdentity: "-"`; Apple Silicon refuses to run unsigned
   code, and there is no Developer ID).

The output:

| What | Where |
|---|---|
| The disk image to share | `target/universal-apple-darwin/release/bundle/dmg/Klat_<version>_universal.dmg` |
| The app | `target/universal-apple-darwin/release/bundle/macos/Klat.app` |
| The hook inside it | `Klat.app/Contents/MacOS/klat-hook`, next to the app's binary: where Settings → Hooks points |

Check the result before sharing it:

```bash
lipo -archs target/universal-apple-darwin/release/bundle/macos/Klat.app/Contents/MacOS/klat-hook   # x86_64 arm64
codesign -dv target/universal-apple-darwin/release/bundle/macos/Klat.app                            # Signature=adhoc
```

The `.dmg` is not notarized, so people who download it have to confirm it once: see
[macos-install.md](macos-install.md). Each build has a new ad-hoc signature, so after an update macOS forgets
the Accessibility permission (the guide explains how to grant it again).

| Build error | Try this |
|---|---|
| `error[E0463]: can't find crate for 'core'` / `target may not be installed` | `rustup target add aarch64-apple-darwin x86_64-apple-darwin` |
| `lipo: can't open input file` | One of the two `cargo build --target` steps failed; scroll up for its error |
| `lipo: command not found` or `xcrun: error` | `xcode-select --install` (lipo comes with the Command Line Tools) |
| `resource path 'binaries/klat-hook-universal-apple-darwin' doesn't exist` | Step 1 did not run; use `pnpm release:mac`, not `pnpm tauri build` |
| `failed to bundle project` while creating the `.dmg` | Close any Finder window showing a mounted Klat disk image and eject it, then build again |

## Checks

```bash
pnpm lint && pnpm typecheck && pnpm test     # frontend
pnpm lint:rust && pnpm test:rust             # Rust (fmt, clippy -D warnings, tests)
pnpm check:mac                                # on a Mac this is a native clippy run
```

## If something goes wrong

| What you see | Try this |
|---|---|
| `xcrun: error: invalid active developer path` | `xcode-select --install`, then build again |
| `pnpm: command not found` | `corepack enable`, open a new terminal |
| The cat has a black or white box around it | Transparency did not take. Check that `src-tauri/tauri.conf.json` has `"macOSPrivateApi": true` and `src-tauri/Cargo.toml` enables the `macos-private-api` feature, then rebuild. Note the macOS version and whether a second monitor is attached |
| Clicks on the cat do nothing, or clicks around it are blocked | Note it with your screen setup (Retina or not, scaling). The click-through maths uses Tauri's physical pixels |
| Space never drags the cat | Settings → General must say **Allowed**. If it does and it still fails, look in `klat.log` for `Space event tap installed` or `could not create the Space event tap` |
| Space stops typing in other apps | It should only be held back while the pointer is over the cat. Quit Klat and report it (with the log) |
| The cat does not wake for Claude Code | Settings → Hooks should say **Installed**; click **Repair** if not, then restart Claude Code sessions. `scripts/send-test-event.sh Stop` with Ready on should wake it. See [hooks-troubleshooting.md](hooks-troubleshooting.md) |
| Settings opens behind other windows | Click it once in Mission Control and note it; menu bar apps sometimes need an extra activation step |
| The menu bar icon is invisible or a black square | Note light/dark menu bar. The icon is `src-tauri/icons/tray-template.png` (a template image; `scripts/make-tray-template.py` draws it) |

## Reporting back

For ClauCat-t0b.12, write down:

- macOS version, chip (Apple Silicon / Intel), screen setup.
- Each step above: worked / did not work, with the exact error text.
- Anything in `~/Library/Application Support/Klat/klat.log` that looks wrong.
- Screenshots of anything that looks off (the cat, the menu bar icon, the Settings window).
