# Claude Cat

Desktop companion for Claude Code on Windows: a grumpy toon cat sleeps in the corner of the screen and yells at you when Claude finishes a turn or is waiting on you. The full design is in [`CLAUDE_CAT_SPEC.md`](CLAUDE_CAT_SPEC.md).

> **Status:** Phase 5 is done: tray → **Hooks → Install** hooks the cat into Claude Code (user-level settings), and the cat wakes when a real Claude Code turn ends. Phase 6 (hardening, release build, measurements) is next.

## Requirements

- Windows 10/11 with the WebView2 runtime (ships with Windows 11)
- Node.js ≥ 20.19, pnpm 8+
- Rust stable (MSVC toolchain) and Visual Studio C++ Build Tools

## Scripts

| Command | What it does |
|---|---|
| `pnpm install` | Install JS dependencies |
| `pnpm tauri dev` | Run the desktop app (Vite dev server and Tauri) |
| `pnpm tauri build` | Build the release app and NSIS installer |
| `pnpm dev` | Run only Vite; the dev page is at http://localhost:1420/dev.html |
| `pnpm build` | Typecheck and build the frontend into `dist/` |
| `pnpm lint` | ESLint |
| `pnpm typecheck` | `tsc --noEmit` |
| `pnpm test` | Vitest |
| `pnpm lint:rust` | `cargo fmt --check` and `cargo clippy -D warnings` (whole workspace) |
| `pnpm test:rust` | `cargo test --workspace` |
| `cargo build -p claude-cat-hook` | Build the hook adapter (`target/debug/claude-cat-hook.exe`) |
| `.\scripts\send-test-event.ps1 <Hook> [sessionId]` | Simulate a Claude Code hook through the real adapter (see below) |

## Dev page (no Tauri needed)

Run `pnpm dev` and open http://localhost:1420/dev.html.

- The page shows the port on the left at the overlay's size (360×280) on a checkerboard, so any transparency problem is visible.
- The prototype runs on the right for side-by-side comparison. It loads three.js from a CDN, so it needs network access.
- The mouse anywhere on the page stands in for the screen cursor.
- **Wake / Sleep / Next phrase / Pet** buttons drive the cat. The dropdown sets the wake reason (`done`, `permission` or `input`), which decides the phrase pool.
- Toggles for SFW, reduce motion, languages, yell interval (6.5s here, as in the prototype; the app default is 30s), max yells, and a dark desktop.
- **Show hitboxes** outlines what the overlay reports as clickable; everything else is click-through.
- Click the cat to pet it; click the bubble to dismiss.
- `window.stage` is exposed in the console, e.g. `stage.bubble.show('text', 'project')`.

## Source layout

| Path | Role |
|---|---|
| `src/cat/springs.ts` | `Spring`, `SpringSet`, `Ballistic` (jump) |
| `src/cat/CatController.ts` | State machine: wake/yell/sleep sequences, blink and ear-twitch schedulers, pet, look. Pure logic. |
| `src/cat/CatRig.ts` | Meshes and `applyPose()`, ported 1:1 from the prototype |
| `src/cat/tail.ts` | Tail tube, preallocated and rewritten in place |
| `src/cat/RenderLoop.ts` | rAF loop with FPS cap that sleeps on a timer between frames; stops while the page is hidden |
| `src/cat/CatStage.ts` | Composes renderer, rig, controller, bubble and loop; reports hitboxes; shared by the overlay and the dev page |
| `src/bridge/tauriBridge.ts` | Typed `cat://state` / `cat://settings` events and commands |
| `src/bubble/Bubble.ts` | Speech bubble (pop, shake, click = dismiss) |
| `src/phrases/` | Phrase data and `PhrasePicker` (shuffle bag, filters) |
| `src/overlay/main.ts`, `dev.ts` | Overlay entry and dev page entry |
| `src-tauri/src/overlay.rs` | Creates the overlay window (transparent, topmost, no taskbar, never activates) |
| `src-tauri/src/tray.rs` | Tray menu |
| `src-tauri/src/cursor.rs` | Cursor poller: click-through off only over the hitboxes; `cat://cursor` for head tracking |
| `src-tauri/src/state.rs` | Shared state: test wake, session store, `cat://state` and `cat://settings` events |
| `src-tauri/src/session_store.rs` | Hook event → attention mapping, per-session pending/dismissed, dedupe, stale. Pure logic. |
| `src-tauri/src/event_server.rs` | Loopback HTTP endpoint, token, `runtime.json` |
| `src-tauri/src/hooks_installer.rs` | Install / repair / uninstall our entries in `~/.claude/settings.json` |
| `crates/claude-cat-hook/` | Hook adapter binary: stdin JSON → POST, always silent, always exit 0 |
| `scripts/send-test-event.ps1` | Fakes Claude Code hook input for manual testing |
| `src-tauri/src/settings.rs` | Settings load/save with field-level fallback and corrupt-file recovery |
| `src-tauri/src/commands.rs` | Commands the overlay invokes |

## Overlay: how to check it yourself

1. Run `pnpm tauri dev`. The cat sleeps in the bottom-right corner, 16px inside the taskbar, with Zzz floating up.
2. Click on empty parts of the 360×280 area. The click goes through to the window underneath; only the cat and the bubble are clickable.
3. Click the sleeping cat. It peeks one eye open, and your terminal keeps focus.
4. Tray icon (it may sit under the ^ hidden-icons arrow) → **Wake test**. The cat jumps up and yells "Wake up, B**CH!!!", then sits grumpy and keeps yelling every 30s.
5. While it is awake, move the mouse anywhere on the screen: the head and pupils follow it. Right after waking the cat looks at its bubble until the mouse moves.
6. Click the bubble, or use tray → **Sleep** / **Dismiss all**. The cat yawns and goes back to sleep.
7. Tray toggles:
   - **SFW mode**: the wake line becomes "Wake up, human!!!".
   - **Hide cat while sleeping**: the cat disappears once asleep and reappears on wake.
   - **Pause notifications**: only saved for now; it starts mattering when hook events arrive in Phase 4.
8. Settings live in `%APPDATA%\ClaudeCat\settings.json`. Tray toggles write it immediately. Hand edits are read at startup.
9. To quit, use tray → **Quit**.

## Hooking into Claude Code

Tray → **Hooks** (the submenu title shows the current status):

- **Install** / **Repair** adds or refreshes one entry per hook event in `%USERPROFILE%\.claude\settings.json`. Before writing, it saves the file as `settings.json.bak.<unix-seconds>`. Your other hooks and settings keep their content and order.
- **Uninstall** removes only Claude Cat's entries (anything whose command contains `claude-cat-hook.exe`).
- A message box reports the result. If `settings.json` is not valid JSON, nothing is written and the box says why.

Each entry uses exec form, which runs the program directly without a shell:

```json
{ "type": "command", "command": "C:\\…\\claude-cat-hook.exe", "args": ["Stop"], "timeout": 1 }
```

The command points at the `claude-cat-hook.exe` next to the running app. In development that is `target\debug\claude-cat-hook.exe`, so run **Repair** after moving or reinstalling the app. When the app is not running, the hook exits in a few ms and Claude Code is unaffected.

## Simulating Claude Code events (no Claude Code needed)

With the app running (`pnpm tauri dev`) and the adapter built (`cargo build -p claude-cat-hook`):

```powershell
.\scripts\send-test-event.ps1 Stop                          # Claude finished a turn -> the cat wakes up
.\scripts\send-test-event.ps1 UserPromptSubmit              # you sent a prompt -> back to sleep
.\scripts\send-test-event.ps1 PermissionRequest -ToolName Bash
.\scripts\send-test-event.ps1 PostToolUse -ToolName Bash    # approved, the tool ran -> sleep
.\scripts\send-test-event.ps1 Notification -NotificationType idle_prompt
.\scripts\send-test-event.ps1 Stop s2 -Cwd C:\code\other   # a second session: the cat yells again and names the project
```

The script feeds Claude Code-style JSON into the real `claude-cat-hook.exe`, so it exercises the whole path.

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

## Phase 5 verification (2026-10-11)

- Unit tests (14) cover the installer:
  - It keeps existing hooks and key order.
  - Installing twice changes nothing.
  - Repair fixes paths and removes duplicates and stale events.
  - Uninstall removes only ours, including when our entry shares a group with theirs, and restores the original.
  - Broken JSON writes nothing and makes no backup.
  - A missing exe is refused before anything is touched.
  - A missing settings file is handled; backups are made; the newline style is kept.
- Dry run on a copy of the real `%USERPROFILE%\.claude\settings.json` (with the user's `rtk` PreToolUse hook): install, then uninstall, came back byte-identical.
- Real install, approved by the user, through tray → Hooks → Install. The message box confirmed it, one backup was written, the `rtk` hook is untouched, and 9 exec-form entries were added.
- Real Claude Code 2.1.296: `claude -p "Reply with exactly: OK"` in a folder `cat-e2e` → the overlay received `wake(done)` (Stop), then `sleep()` 30ms later (SessionEnd).
- Still to confirm interactively: a reply finishing in an open session wakes the cat; a permission prompt wakes it right away; the next prompt puts it to sleep.

## Phase 4 verification (2026-10-10)

- Tests: 43 app tests (session store mapping, dedupe, isolation between sessions, dismiss, pause, stale; HTTP parsing and every rejection; real-socket round trip; token; runtime file) and 14 hook tests (field extraction, no leaks, truncated input, basename, timeouts; 5 that run the real binary).
- End to end with the real app and `send-test-event.ps1` → `claude-cat-hook.exe` → server:
  - Stop wakes the cat with "Wake up, B**CH!!!". A duplicate within 2s is ignored.
  - A second session's Stop makes the cat yell again with the project name. Each session's UserPromptSubmit clears only that session.
  - PermissionRequest(Bash) wakes the cat. A later `permission_prompt` is ignored. PostToolUse(Edit) does not clear the wait; PostToolUse(Bash) does.
  - `idle_prompt` wakes the cat; dismiss puts it to sleep; PreToolUse and SubagentStop change nothing.
- Live server probing with curl: valid 204; wrong or missing token 401; 70 KB body 413; unknown hook 422; broken JSON 400; `Origin` 403; GET 405; other path 404. It listens on 127.0.0.1 only.
- Hook timing with the app not running: 3–4 ms with no `runtime.json` (Tray → Quit deletes it), and ~160 ms with a stale `runtime.json` pointing at a closed port. Windows takes up to ~1s to refuse a loopback connection; the 150ms connect cap cuts that short.

## Phase 3 verification (2026-10-10)

- Head tracking (100%, screenshots): the head and pupils turn towards the cursor at top-left, top-right, just left of the cat, screen centre and below the cat.
- Events: ~29/s while the mouse moves, 0/s while it is still. The overlay point (100, 50) arrives as `100,50`.
- Cost: the 30Hz poller uses ~0.3% of one core in `claude-cat.exe`. Total app CPU while awake is unchanged at ~6%.
- DPI: 100% passes the automated checks. The user switched to 125% with the app running, saw the cat correctly placed and sized, and switched back. After the round trip the window was pinned exactly in the corner again (16px gaps), which exercises the re-pin. The placement maths for 125% and 150%, and the cursor conversion at both, are unit-tested.
- Not tested on real hardware: 150% (only unit tests) and a second monitor (this machine has one).

## Phase 2 verification (2026-10-10, debug build under `pnpm tauri dev`, 2560×1440 at 100%, 165Hz, 20 logical cores)

| Check | Result |
|---|---|
| Window | 360×280 at the work-area corner; `WS_EX_TOPMOST` and `WS_EX_NOACTIVATE` are set |
| Click-through | Over empty overlay area the window under the cursor is the editor (`WS_EX_TRANSPARENT` on). Over the cat or the bubble it is the overlay (off). It switches back after dismiss. |
| Focus | With the editor focused, clicking the cat and clicking the bubble both leave the editor in the foreground |
| Tray | Wake test → wakes and yells; bubble click / Sleep → yawns and sleeps; SFW → "Wake up, human!!!"; Hide cat while sleeping → vanishes after the yawn and returns on wake; `settings.json` updates |
| FPS cap | Render 20fps asleep and 60fps awake (counted with WebView2 remote debugging) |
| CPU | Asleep 3.4% of one core (0.17% of total CPU); awake 7.4% of one core; hidden ≈ 0. Before the two loop fixes, asleep was 11.4%. |
| Memory | 204 MB private (app 5 MB, WebView2 GPU 90, renderer 55, browser 31, others 22); 397 MB working set. This is a dev build with Vite HMR, so the real measurement is in Phase 6. **It may exceed the 150 MB target**, because WebView2's processes alone take most of it. |

Known limitation: right after you use the tray, the foreground is a short-lived shell window. Clicking the bubble in that moment can make Windows hand focus to the overlay. With the terminal focused (the normal case), focus stays where it is.

Not checked yet: DPI scaling at 125/150% and multiple monitors (Phase 3). The hit-test maths is unit-tested at 150%.

## Phase 1 verification (2026-10-10)

- `pnpm typecheck`, `pnpm lint` and `pnpm test` (62 tests) pass.
- Side-by-side screenshots of the dev page against the prototype (headless Edge) for sleep, wake/yell, active, pet, look, yawn and asleep-again. Shape, colours and poses match.
- Allocation profiling with the heap sampling profiler, 5s while ACTIVE and following a moving cursor:
  - Prototype: ~10.5 MB/s, mostly from rebuilding `TubeGeometry` every frame.
  - Port: ~0.47 MB/s. All of the top allocators are inside `THREE.WebGLRenderer.render` (`updateMatrixWorld`, render-list sort), which V8 needs to box floats for. None of them are in `src/`.
  - A bare rAF loop allocates 0 B/frame on the same browser.
- Reduce motion: during wake, max |jumpY| and |squash| are ≈0, and the bubble has no shake class.
- The overlay window shows the sleeping cat, transparent over other windows, with Zzz floating.

## Phase 0 verification (2026-10-10, Windows 11 Pro 22621, WebView2 154, 2560×1440 at 100%)

| Check | Result | How |
|---|---|---|
| Transparent background | ✅ | Screenshot: pixels in the window but outside the sphere match the window behind it, and `alpha: true` + `setClearColor(0, 0)` works without any workaround |
| No border, title bar or shadow | ✅ | Screenshot |
| Not on the taskbar | ✅ | Taskbar screenshot shows no button |
| Always on top | ✅ | `WS_EX_TOPMOST` is set |
| Position | ✅ | Window rect (2184,1096)-(2544,1376): 360×280, 16px inside the work area (2560×1392) |
| Does not steal focus | ✅* | The foreground window stayed on the editor after launch. *Only checked at 100% DPI and on one launch. Phase 2 re-checks this together with click-through. |

Not checked yet: DPI scaling at 125/150% and multiple monitors (Phase 3).
