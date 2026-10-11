# Verification log

What was checked at the end of each phase, how, and on what machine. Newest first. The decisions behind these results are in the README's decisions log.

## Phase 6 verification (2026-10-11, release build)

- `pnpm release` builds the hook (release), stages the sidecar, then builds the app and `Claude Cat_0.1.0_x64-setup.exe` (2.13 MB). The NSIS script installs `claude-cat-hook.exe` into `$INSTDIR` next to `claude-cat.exe`.
- Silent install (`/S /D=<temp>`) exits 0. It creates `claude-cat.exe`, `claude-cat-hook.exe` and `uninstall.exe`, plus an HKCU uninstall entry.
- **Single instance:** launching the installed app a second time leaves one process, shows "Claude Cat is already running…", and logs `second launch ignored`.
- **Start at login:** ticking it in the tray creates `HKCU\…\Run\Claude Cat = <install dir>\claude-cat.exe` and sets `startAtLogin: true`. Unticking removes the value and sets `false`.
- **Installed sidecar:** piping a Stop JSON into the installed `claude-cat-hook.exe` exits 0, and the app logs `hook Stop … -> attention Done`. A UserPromptSubmit carrying a prompt is logged as `-> cleared`, and the prompt text appears nowhere in the log files.
- Tray → Quit logs `exiting` and deletes `runtime.json`. The silent uninstall exits 0 and removes the install folder and the uninstall entry.
- **CPU/RAM (release):** asleep ~0.35% of total CPU (WebView2 GPU 3.4% + renderer 3.3% of one core); awake ~0.55%; 185–189 MB private. Asleep there are 20 rAF/s and ~48 draw calls per frame. WebView2 flag experiments are under the README's Performance section.
- Tests: 69 TS, 75 Rust (60 app incl. 3 log tests and 14 installer tests, 9 hook unit, 5 hook binary, plus 1 ignored dry run).

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
