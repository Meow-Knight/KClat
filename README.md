# Claude Cat

A desktop companion for Claude Code on Windows. A grumpy toon cat sleeps in the corner of your screen. When Claude finishes a turn or is waiting on you (a permission prompt, a question), it jumps up and yells "Wake up, B\*\*CH!!!" until you go back to the terminal. It follows your cursor, and you can pet it.

Everything runs locally: no telemetry, no cloud. The hooks only report to the app over `127.0.0.1` and never approve or deny anything for you. The full design is in [`CLAUDE_CAT_SPEC.md`](CLAUDE_CAT_SPEC.md).

> **Status:** MVP complete (spec phases 0–6).

## Install

1. Run `Claude Cat_<version>_x64-setup.exe`. It is a per-user install into `%LOCALAPPDATA%\Claude Cat`, so no admin rights are needed. The installer contains `claude-cat.exe` and the hook program `claude-cat-hook.exe`.
2. Start **Claude Cat**. A sleeping cat appears in the bottom-right corner, and a cat icon appears in the tray (it may be under the **^** hidden-icons arrow).
3. Tray → **Hooks → Install**. This adds Claude Cat's hooks to `%USERPROFILE%\.claude\settings.json`, after saving a backup. Your other hooks and settings stay as they are.
4. Restart any open Claude Code sessions. From now on the cat wakes when Claude needs you.

You need Windows 10/11 with the WebView2 runtime, which comes with Windows 11 and recent Windows 10.

## Using it

| When | The cat |
|---|---|
| Claude finishes a turn, asks for permission, or waits for input | Jumps up and yells, then glares and taps its paw; it yells again every 30s (10 times at most) |
| More than one session needs you | Yells again for each new one and names the project under the phrase |
| You send a prompt, approve the permission, or the session ends | Yawns and goes back to sleep |
| You click the bubble | Goes back to sleep. This marks those sessions as seen and sends nothing to Claude Code |
| You click the cat | Asleep: peeks one eye open. Awake: purrs (^ ^) |
| You move the mouse while it is awake | Follows the cursor anywhere on the main screen |

The overlay never takes focus, and clicks on its empty area go through to the window below.

**Tray menu:**

| Item | What it does |
|---|---|
| Wake test | Wakes the cat without Claude Code |
| Sleep / Dismiss all | Puts the cat to sleep and marks waiting sessions as seen |
| SFW mode | No swearing (the first line becomes "Wake up, human!!!") |
| Pause notifications | The cat stays asleep. Sessions are still tracked, and unpausing wakes it if something is still waiting |
| Hide cat while sleeping | The cat shows only while it is awake |
| Start at login | Off by default. Adds or removes Claude Cat in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| Hooks → Install / Repair / Uninstall | Manage the entries in `settings.json` (see [Hooks](#hooks)). The submenu title shows the current status |
| Quit | Quits the app |

The tray tooltip shows how many sessions are waiting.

**Settings** are in `%APPDATA%\ClaudeCat\settings.json`. Tray toggles write the file immediately. Hand edits are read at start-up. A missing or invalid field falls back to its default. A file that is not JSON at all is renamed to `settings.json.corrupt.<time>` and replaced with defaults.

| Field | Default | |
|---|---|---|
| `notificationsEnabled` | `true` | `false` = paused |
| `hideWhileSleeping` | `false` | |
| `sfwMode` | `false` | |
| `languages` | `["en","vi"]` | Phrase languages; the first wake line is always English |
| `yellIntervalMs` | `30000` | ≥ 1000 |
| `maxYellsPerWake` | `10` | Includes the first yell |
| `staleAfterMs` | `1800000` | A session waiting longer than this stops counting (≥ 60000) |
| `reduceMotion` | `false` | No jumps, squash, puff, text shake, purr shake or Zzz; springs are critically damped |
| `startAtLogin` | `false` | Use the tray item; it keeps the registry in sync |
| `corner` | `"bottom-right"` | The only corner supported so far |

## Hooks

**Hooks → Install** adds one entry per event (`Stop`, `StopFailure`, `PermissionRequest`, `Notification`, `UserPromptSubmit`, `PostToolUse`, `PostToolUseFailure`, `PermissionDenied`, `SessionEnd`) to `%USERPROFILE%\.claude\settings.json`. Each entry looks like this:

```json
{ "type": "command", "command": "C:\\Users\\you\\AppData\\Local\\Claude Cat\\claude-cat-hook.exe", "args": ["Stop"], "timeout": 1 }
```

- It runs the program directly (exec form), with no shell involved, so paths with spaces are fine.
- Before any change, the file is saved as `settings.json.bak.<unix-seconds>`.
- If `settings.json` is not valid JSON, nothing is written and a message box says so.
- Installing again changes nothing. **Repair** re-points the entries after you move or reinstall the app, or switch between a dev build and the installed one.
- **Uninstall** removes only entries whose command contains `claude-cat-hook.exe`, and keeps everything else, including key order.
- The app never installs or changes hooks on its own; only these tray items do.

The hook reads the event from stdin and forwards five fields: event name, session id, project folder name, tool name and notification type. It sends them to `127.0.0.1`, prints nothing, and always exits 0. Its budget is 300ms. When the app is not running, it exits in a few milliseconds.

## Uninstall

1. Tray → **Hooks → Uninstall**, then **Quit**.
2. Windows Settings → Apps → **Claude Cat** → Uninstall.
3. Optional: delete `%APPDATA%\ClaudeCat` (settings and log) and any `%USERPROFILE%\.claude\settings.json.bak.*` backups you no longer need.

If you uninstall the app first, the hook entries stay in `settings.json`. They are harmless (the program is gone, so the hook just fails quietly), but you can delete them by hand: they are the entries whose `command` contains `claude-cat-hook.exe`.

## Privacy

- **Nothing leaves your machine.** The app listens only on `127.0.0.1`, on a random port, with a fresh 256-bit token on each run. `%APPDATA%\ClaudeCat\runtime.json` holds the port and token while the app runs and is deleted when you quit.
- The server rejects anything without the token, any request with an `Origin` header (web pages), bodies over 64 KB, and unknown event names.
- The hook never sends prompts, commands, tool input or output, messages, transcript paths or full folder paths. It sends only the five fields listed under [Hooks](#hooks).
- The local log (`%APPDATA%\ClaudeCat\claude-cat.log`, capped at 512 KB plus one `.old` file) records start-up, hook event names with the first 8 characters of the session id and the project folder name, rejected requests, and installer and autostart actions.
- The hook never makes permission decisions.

## Troubleshooting

| Problem | Try |
|---|---|
| The cat doesn't wake when Claude finishes | Check the tray: does it say **Hooks (installed)**? If not, run Install or Repair. Then restart the Claude Code session. See [docs/hooks-troubleshooting.md](docs/hooks-troubleshooting.md) for step-by-step checks and how to read the log |
| "Hooks (needs repair)" | The entries point at another copy of `claude-cat-hook.exe` (moved or reinstalled app, dev vs installed): run **Repair** |
| Install says settings.json is invalid | Fix the JSON, or restore a `settings.json.bak.*` backup; nothing was changed |
| The cat stays awake | Send a prompt or click the bubble. A session waiting longer than 30 minutes is dropped automatically |
| Opening the app again shows "already running" | Only one cat runs at a time. Look in the tray |
| Black or white box instead of a transparent window | Update the WebView2 runtime and your GPU driver |

## Known limitations

- Windows only. The cat lives in the bottom-right corner of the **primary** monitor's work area. With the cursor on another monitor, the cat looks at its bubble instead of following.
- Tested at 100% and 125% display scaling. 150% and multi-monitor setups are covered only by unit tests so far.
- **Memory:** ~185 MB private (release build, 7 processes; about 90 MB of it is the WebView2 GPU process). That is over the spec's ~150 MB goal; WebView2 flags saved only ~10 MB (see [Performance](#performance)).
- A `PermissionDenied` hook exists only in auto mode. If you deny a permission by hand, the cat stays awake until your next prompt.
- `claude -p` (print mode) ends its session at once, so the cat wakes and falls asleep again within milliseconds.
- Right after you use the tray menu, clicking the bubble can give focus to the overlay (Windows hands focus over from the closing tray window). With your terminal focused, it never takes focus.
- The hitbox is a generous box around the cat rather than its exact outline, so clicks just next to the cat can land on it.
- The Windows uninstaller does not remove the hooks from `settings.json`; use **Hooks → Uninstall** first.

## Performance

Release build, Windows 11, 2560×1440 at 100%, 165Hz display, 20 logical cores (2026-10-11):

| | CPU | Memory |
|---|---|---|
| Asleep (20fps) | ~0.35% of total CPU (~7% of one core: WebView2 GPU ~3.4% + renderer ~3.3%; the app itself ~0%) | 185 MB private / ~360 MB working set |
| Awake (60fps) | ~0.55% of total CPU (~11% of one core) | 189 MB private |
| Hidden ("Hide cat while sleeping") | ≈ 0 | same |

Asleep, the page wakes 20 times a second (it does not wake on every vsync), with ~48 draw calls per frame. The cursor poller costs ~0.3% of one core while awake. The installer is 2.1 MB; the app exe is 9.2 MB and the hook 247 KB.

Memory by process (private): WebView2 GPU 89 MB, renderer 36, browser 34, utility 18, crashpad 3, `claude-cat.exe` 6. WebView2 flags tried: `--in-process-gpu` saved ~10 MB at the cost of coupling GPU crashes to the browser process; `--renderer-process-limit=1` and disabling the GPU shader cache saved nothing. The defaults were kept.

## Development

Requirements: Node.js ≥ 20.19, pnpm 8+, Rust stable (MSVC) with the Visual Studio C++ Build Tools.

| Command | What it does |
|---|---|
| `pnpm install` | Install JS dependencies |
| `pnpm tauri dev` | Run the app (Vite dev server and Tauri, hot reload) |
| `pnpm release` | Build the hook (release), stage it as a sidecar, build the app and the NSIS installer → `target/release/bundle/nsis/` |
| `pnpm dev` | Vite only; the dev page is at http://localhost:1420/dev.html |
| `pnpm build` | Typecheck and build the frontend into `dist/` |
| `pnpm build:hook` | Build `claude-cat-hook` (release) and copy it to `src-tauri/binaries/` |
| `pnpm lint` / `pnpm typecheck` / `pnpm test` | ESLint / `tsc --noEmit` / Vitest |
| `pnpm lint:rust` / `pnpm test:rust` | `cargo fmt --check` + `clippy -D warnings` / `cargo test --workspace` |
| `cargo build -p claude-cat-hook` | Debug build of the hook (`target/debug/claude-cat-hook.exe`, next to the dev app) |
| `.\scripts\send-test-event.ps1 <Hook> [sessionId]` | Simulate a Claude Code hook through the real adapter |

`pnpm tauri build` without the release config builds the app without the bundled hook. Use `pnpm release`, which adds `src-tauri/tauri.release.conf.json` (sidecar plus `build:hook`).

### Dev page (no Tauri needed)

Run `pnpm dev` and open http://localhost:1420/dev.html.

- The port runs on the left at the overlay's size (360×280) on a checkerboard; the prototype runs on the right for comparison (it needs network access for three.js).
- The mouse stands in for the screen cursor.
- **Wake / Sleep / Next phrase / Pet** buttons drive the cat, plus toggles for SFW, reduce motion, languages, yell interval (6.5s here), max yells, a dark desktop, and **Show hitboxes**.
- `window.stage` is exposed in the console.

### Simulating Claude Code events

With the app running and the hook built:

```powershell
.\scripts\send-test-event.ps1 Stop                          # Claude finished a turn -> the cat wakes up
.\scripts\send-test-event.ps1 UserPromptSubmit              # you sent a prompt -> back to sleep
.\scripts\send-test-event.ps1 PermissionRequest -ToolName Bash
.\scripts\send-test-event.ps1 PostToolUse -ToolName Bash    # approved, the tool ran -> sleep
.\scripts\send-test-event.ps1 Notification -NotificationType idle_prompt
.\scripts\send-test-event.ps1 Stop s2 -Cwd C:\code\other   # a second session: yells again and names the project
```

### Source layout

| Path | Role |
|---|---|
| `src/cat/springs.ts` | `Spring`, `SpringSet`, `Ballistic` (jump) |
| `src/cat/CatController.ts` | State machine: wake/yell/sleep sequences, blink and ear-twitch schedulers, pet, look. Pure logic |
| `src/cat/CatRig.ts` | Meshes, Zzz sprites and `applyPose()`, ported from the prototype |
| `src/cat/tail.ts` | Tail tube, preallocated and rewritten in place |
| `src/cat/RenderLoop.ts` | FPS-capped rAF loop that sleeps on a timer between frames and stops while hidden |
| `src/cat/CatStage.ts` | Composes renderer, rig, controller, bubble and loop; reports hitboxes |
| `src/bubble/Bubble.ts` | Speech bubble |
| `src/phrases/` | Phrase data and `PhrasePicker` |
| `src/bridge/tauriBridge.ts` | Typed events and commands |
| `src/overlay/main.ts`, `dev.ts` | Overlay entry, dev page entry |
| `src-tauri/src/overlay.rs` | Overlay window (transparent, topmost, no taskbar, never activates; DPI-correct placement) |
| `src-tauri/src/tray.rs` | Tray menu, autostart, hook installer UI |
| `src-tauri/src/cursor.rs` | Cursor poller: click-through and `cat://cursor` |
| `src-tauri/src/state.rs` | Shared state, `cat://state` / `cat://settings` |
| `src-tauri/src/session_store.rs` | Hook event mapping and per-session state. Pure logic |
| `src-tauri/src/event_server.rs` | Loopback HTTP endpoint, token, `runtime.json` |
| `src-tauri/src/hooks_installer.rs` | Install / repair / uninstall in `~/.claude/settings.json` |
| `src-tauri/src/settings.rs` | Settings load/save |
| `src-tauri/src/log.rs` | Capped local log |
| `crates/claude-cat-hook/` | Hook adapter |
| `scripts/` | `send-test-event.ps1`, `stage-sidecar.mjs` |
| `docs/` | Prototype, [hooks troubleshooting](docs/hooks-troubleshooting.md), [verification log](docs/verification.md) |

## Decisions log

Small decisions made during implementation, as the spec asks (§11).

- **Phase 0**
  - Pinned versions: Tauri 2.12 (crate `tauri` 2.12.2, CLI 2.12.1), Vite 8.3.4, Three.js 0.186.1, TypeScript 6.0.3, Vitest 4.1.11, ESLint 10.
    - TypeScript 7 is not supported by `typescript-eslint` yet (it needs a version below 6.1).
    - Vitest 5 needs Node 22+, and this machine runs Node 20.19.
  - The overlay window is created in Rust (`src-tauri/src/overlay.rs`) rather than declared in `tauri.conf.json`. This way it is placed at the work-area corner and created with `focused(false)`, so it never needs a `show()` call that could steal focus.
  - A minimal tray with only **Quit** is already included, because the overlay has no taskbar button and no decorations. The full tray menu (§2.3) comes in Phase 2.
  - The app icon is a generated placeholder cat head (`src-tauri/icons/`).
  - The bundle identifier is `vn.ema.claudecat`.
  - The spike uses `THREE.Timer` instead of `THREE.Clock`, which is deprecated in three r186.

- **Phase 1**
  - `maxYellsPerWake` counts every yell of a wake, including the first "Wake up" yell. With the default of 10, the cat yells the wake phrase and then 9 more times before it sits silent.
  - `yellNow()` is the "another session became pending" path. It yells immediately even after the cap is reached, wakes the cat if it is asleep, and restarts the ignore timer so two yells never land back to back.
  - A yell's 1.1s "settle into the squint" timer is replaced by the next yell, so it cannot close the new yell's mouth early (the prototype could).
  - `sleep()` clears the happy face and puff. In the prototype, a pet right before sleep left the ^ ^ eyes on forever, because sleep cancelled the timer that resets them.
  - On wake the cat first looks at the bubble (`lx=-.35, ly=-.08`, §6.4) until cursor data arrives.
  - Wake phrases skip the language filter, because there is no Vietnamese wake phrase and the first yell must always happen. If the filters leave the pool empty, the picker falls back to the wake phrase.
  - When the pool changes (the reason changes, or `ignored` phrases join from the 3rd yell), a new shuffle bag starts. It never opens with the phrase just said.
  - The tail evaluates its centripetal Catmull-Rom spline itself instead of calling `THREE.CatmullRomCurve3.getPoint`. Three's version boxes a float per coefficient and was the biggest allocator left in the frame. A test checks that the result matches three's curve to 1e-4.
  - The tail samples the spline uniformly by parameter, not by arc length (the prototype's `TubeGeometry` used arc length). The difference is not visible, and arc-length sampling allocates.
  - Camera framing for 360×280: camera at (0.5, 2.3, 11.5) looking at (0.5, 2.0, 0). This puts the cat on the right with room for the bubble on the left. The prototype used z = 10 in a wider 680px stage.
  - Toon materials are shared per colour instead of one per mesh.

- **Phase 2**
  - The tray item "Hide cat" (§2.3) is labelled **Hide cat while sleeping** and toggles `hideWhileSleeping`, the only hiding setting in §10.
  - `hideWhileSleeping` does not call `window.hide()`/`show()`. tao shows a window with `SW_SHOW` after creation, and that can take focus. Instead the overlay stays up, the stage hides with CSS, the render loop stops, and no hitboxes are reported, so the whole window is click-through. It looks the same and costs ~0 CPU.
  - The overlay is created with `focusable(false)` (`WS_EX_NOACTIVATE`), so clicking the cat or the bubble does not take focus from the terminal.
  - Hitboxes are static, generous world-space boxes around the cat for each pose (awake and asleep), projected to screen px, plus the bubble's layout box while it shows. They are sent to Rust only when the layout changes (bubble shows or hides, pose phase changes, resize), not every frame.
  - The cursor poller is a plain thread: 30Hz while awake, 5Hz while asleep. It flips `set_ignore_cursor_events` only when the inside/outside state changes.
  - **Sleep** and **Dismiss all** do the same thing for now (clear the Wake test). Phase 4 makes Dismiss mark sessions as dismissed.
  - **Wake test** works even while notifications are paused, because it is an explicit manual action.
  - Settings use field-level fallback: a mistyped or out-of-range field falls back to its own default and the others are kept. Only a file that is not a JSON object counts as corrupt. Unknown fields are dropped on the next save. Limits: `yellIntervalMs` ≥ 1000, `maxYellsPerWake` ≥ 1, `staleAfterMs` ≥ 60000, languages must be from `en`/`vi` (an empty list means the default), and the only supported corner is `bottom-right`.
  - Settings are saved atomically (temp file, then rename).
  - The overlay page exposes `window.stage` in dev builds only, for WebView2 remote debugging.
  - **Render loop:** between frames it now sleeps on a timer and only then requests a frame, instead of skipping vsyncs. On the 165Hz test display this cut rAF wake-ups from 165/s to 20/s asleep and 80/s awake.
  - **Zzz:** they moved from a CSS animation to sprites in the WebGL scene. The CSS animation ran at the display refresh rate and, by itself, cost ~8% of a core while the cat slept.

- **Phase 3**
  - `cat://cursor` is sent only while the cat is awake and only when the cursor moved (≤30Hz). Right after a wake nothing is sent until the cursor moves, so the cat first looks at its bubble (§6.4).
  - The payload is logical px relative to the overlay's top-left corner. Because the stage fills the window, the frontend uses it as is: dx/dy are measured from the projected head position, and `lookFromOffset` turns them into angles.
  - "Cursor on another monitor" means outside the bounds of the overlay's monitor. The event then carries `null`, and the cat looks at its bubble.
  - The overlay is positioned in physical px from its real outer size and the work area (margin = 16 × scale). Creation uses a logical estimate, and `place()` corrects it. On `ScaleFactorChanged` it is re-pinned 150ms later, after Windows has applied the new size.

- **Phase 4**
  - Rust is now a Cargo workspace (`src-tauri`, `crates/claude-cat-hook`) with one `Cargo.lock` and one `target/` at the root.
  - The event mapping was checked against the official hooks docs (2026-10). It differs from the §8.1 sketch in these ways:
    - `Notification` is a fallback only. It is ignored when the session is already waiting. `permission_prompt` fires ~6s after `PermissionRequest`, and `idle_prompt` ~60s after `Stop`, so neither re-wakes a cat you already dismissed.
    - Besides `idle_prompt`, `agent_needs_input`, `elicitation_dialog` and `elicitation_url_dialog` also mean "needs input".
    - `StopFailure` (a turn that ended on an API error) counts as done. `PostToolUseFailure` clears a permission wait like `PostToolUse` does, because the tool ran, so you approved it.
    - `SubagentStop` is ignored, since the main agent is still working.
    - `PermissionDenied` only fires in auto mode. When you deny manually, Claude replies and `Stop` moves the session to "done".
  - The 2s dedupe applies to the same session and the same reason. A permission wait is cleared only by the tool that asked for it, or by any tool when the wait came from a Notification fallback with no tool name.
  - A second session needing you while the cat is awake does not re-run `wake()`. `cat://state` carries an `attention.seq` that changes on every new attention event, and the overlay then calls `yellNow()`. When more than one session is waiting, the bubble's second line names the project (§7.2 rule 5), and the tray tooltip shows the count. Both are Phase 6 items that came almost free here.
  - Pause notifications: while paused, sessions are still tracked but the cat stays asleep and the list sent to the overlay is empty. Unpausing wakes the cat if something is still waiting. Stale waits (default 30 min) are checked every 30s.
  - The server is plain `std::net` (no HTTP crate), with one thread per connection and at most 16 at a time. It rejects requests with an `Origin` header, which shuts out browser pages even if they guessed the port, and it rejects `Transfer-Encoding`. Status codes: 401 token, 413 body > 64 KB (checked before reading the body), 422 unknown hook or bad fields (≤ 200 chars each), 400 malformed, 404/405 wrong path or method.
  - The token is 32 bytes from the OS RNG (`getrandom`), compared in constant time. `runtime.json` (`port`, `token`, `pid`) is written atomically and deleted on a normal quit. A stale file left by a kill is overwritten on the next start.
  - **Adapter:**
    - Its only dependency is `serde_json`. It reads at most 256 KB of stdin. If the JSON is cut off (a huge `tool_response`), it scans the prefix for the few string fields it needs; Claude Code writes those first.
    - It sends `projectName` as the basename of `cwd`, never the path. It never sends tool input, commands, messages or transcripts.
    - Budget: 300ms in total (connect ≤ 150ms). It prints nothing, exits 0 even on a panic, and the hook name argument is only a fallback for `hook_event_name`.
  - No log file yet: nothing is logged, so nothing sensitive can be. A capped debug log is left for Phase 6.

- **Phase 5**
  - Entries use **exec form** (`command` = absolute exe path, `args` = [event]) rather than §8.5's quoted command string. On Windows, Claude Code runs shell-form hooks through Git Bash, or PowerShell when Git Bash is missing, and the two need different quoting (PowerShell needs `& "path"`). Exec form has no shell, so a path with spaces is just a JSON string. Verified in the official docs (2026-10) and against Claude Code 2.1.296.
  - Nine events are installed, matching the Phase 4 mapping. `SubagentStop` is not installed. Entries under events we no longer use are removed on install/repair.
  - **Install** and **Repair** run the same idempotent operation. An existing entry of ours is updated in place, extra copies are removed, and a missing one is appended as its own group.
  - **Uninstall** removes groups and event lists only if removing our entries emptied them. Empty structures that were already there stay.
  - Key order is preserved with `serde_json`'s `preserve_order` feature. The trailing-newline style of the file is kept. A dry run on a copy of the real settings file (install, then uninstall) came back byte-identical.
  - Write safety:
    - The exe must exist before anything is written.
    - The backup is taken before writing.
    - The file is written to a temp file and renamed into place.
    - If `settings.json` changed on disk while we worked (Claude Code writes it too), the write is refused.
    - A no-op install neither writes nor makes a backup.
  - The installer never runs at start-up. The tray only reads the file to show the status.
  - Results are shown with `tauri-plugin-dialog` message boxes (the official plugin).
  - A `claude -p` run ends its session right away, so the cat wakes on `Stop` and sleeps on `SessionEnd` ~30ms later. Interactive sessions stay open, so the cat stays awake until the next prompt.

- **Phase 6**
  - **Single instance** uses `tauri-plugin-single-instance`, registered first. A second launch exits before creating any window, and the running app shows "already running". Two instances used to fight over `runtime.json` and the WebView2 profile, and the second one crashed.
  - **Start at login** uses `tauri-plugin-autostart`, which writes the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `Claude Cat`. It is off unless you tick it. At start-up the Run key is treated as the truth (you may have disabled it in Task Manager) and `startAtLogin` is synced to it.
  - **Log:** `%APPDATA%\ClaudeCat\claude-cat.log`, rotated to `.old` at 512 KB, one line per entry, each line at most 300 characters. It records names and outcomes only: 8 characters of the session id, the project folder name, event names, HTTP status of rejected requests, installer and autostart results. In debug builds lines also go to stderr.
  - **Bundling:** `claude-cat-hook` ships as a Tauri sidecar (`bundle.externalBin`), which the installer places next to `claude-cat.exe` as `claude-cat-hook.exe`. That is exactly where the hooks installer points. The sidecar config lives in `src-tauri/tauri.release.conf.json` (used by `pnpm release`), because tauri-build would otherwise require the staged binary for every dev build. In dev the workspace build puts the hook in `target/debug` next to the app.
  - The NSIS installer is per-user (`%LOCALAPPDATA%\Claude Cat`) and needs no admin rights. Uninstalling does not touch `settings.json` hooks or `%APPDATA%\ClaudeCat`.
  - Memory (release, 185 MB private) is over the ~150 MB goal. `--in-process-gpu` saved ~10 MB but ties GPU crashes to the browser process, so WebView2 keeps its defaults. The numbers are recorded under Performance.
  - The README is reorganised for users first (install, use, hooks, uninstall, privacy, troubleshooting, limitations, performance), then development. Per-phase verification moved to [docs/verification.md](docs/verification.md).
