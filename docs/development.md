# Developing Klat

Building, testing and releasing Klat, the source layout, measurements and the decisions log. Read [ARCHITECTURE.md](ARCHITECTURE.md) before adding a feature.

## Performance

Release build, Windows 11, 2560×1440 at 100%, 165Hz display, 20 logical cores (2026-10-11):

| | CPU | Memory |
|---|---|---|
| Asleep (20fps) | ~0.35% of total CPU (~7% of one core: WebView2 GPU ~3.4% + renderer ~3.3%; the app itself ~0%) | 185 MB private / ~360 MB working set |
| Awake (60fps) | ~0.55% of total CPU (~11% of one core) | 189 MB private |
| Hidden ("Hide cat while sleeping") | ≈ 0 | same |

Asleep, the page wakes 20 times a second (it does not wake on every vsync), with ~48 draw calls per frame. The cursor poller costs ~0.3% of one core while awake. The installer is 2.1 MB; the app exe is 9.2 MB and the hook 247 KB.

Memory by process (private): WebView2 GPU 89 MB, renderer 36, browser 34, utility 18, crashpad 3, `klat.exe` 6. WebView2 flags tried: `--in-process-gpu` saved ~10 MB at the cost of coupling GPU crashes to the browser process; `--renderer-process-limit=1` and disabling the GPU shader cache saved nothing. The defaults were kept.

Memory after ClauCat-gwd (release, 2026-10-14). "Task Manager" is its Memory column (private working set); "committed" is private bytes:

| | Task Manager | Committed |
|---|---|---|
| Before: asleep | ~117 MB | ~183 MB |
| Asleep, after start-up | ~26 MB | ~175 MB |
| Awake | ~52 MB | ~173 MB |
| Asleep again | ~50 MB | ~178 MB |

Two changes, both in `platform/webview_memory.rs`. The network service runs inside the browser process (`NetworkServiceInProcess2`; the page only talks to the app over IPC), which removes a ~11 MB process. And 3 s after the cat falls asleep, the overlay sets WebView2's memory target to low, which trims the working sets of all its processes; it goes back to normal when the cat wakes. CPU is unchanged (asleep ~3–7% of one core, awake ~11%).

Also measured and not used: the GPU process is ~81 MB even without WebGL (`--disable-webgl`), so it is Chromium's compositor, not the cat. `--disable-gpu` brings the total down to ~141 MB committed, but WebGL then runs on WARP and the sleeping cat costs ~20% of one core instead of ~4%. Turning off antialiasing saves ~5 MB and makes the cat jagged.

## Development

Requirements: Node.js ≥ 20.19, pnpm 10.34.6+ (`corepack enable` picks the pinned version; older pnpm has known CVEs), Rust stable (MSVC) with the Visual Studio C++ Build Tools. On macOS: see [docs/macos-build.md](macos-build.md).

| Command | What it does |
|---|---|
| `pnpm install` | Install JS dependencies |
| `pnpm tauri dev` | Run the app (Vite dev server and Tauri, hot reload) |
| `pnpm release` | Build the hook (release), stage it as a sidecar, build the app and the NSIS installer → `target/release/bundle/nsis/`, plus `SHA256SUMS.txt` to publish with it |
| `pnpm dev` | Vite only; the dev page is at http://localhost:1420/dev.html |
| `pnpm build` | Typecheck and build the frontend into `dist/` |
| `pnpm release:mac` | macOS only: universal `klat-hook` (lipo), then the universal app and `.dmg` → `target/universal-apple-darwin/release/bundle/dmg/` ([details](macos-build.md#building-the-dmg)). Without a Mac, the GitHub workflow **Release** runs it ([Publishing a release](#publishing-a-release)) |
| `pnpm build:hook` | Build `klat-hook` (release) for the target being built and copy it to `src-tauri/binaries/` |
| `pnpm check` | Every gate below plus `cargo audit`, then a pass/fail table; exits 1 if any fails. `--mac` adds `check:mac`, `--skip audit` for offline, `--fail-fast`, `--verbose` |
| `pnpm regress` | Windows: starts the debug app in a temp data folder and checks its behaviour end to end through the real hook (quit an installed Klat first); see [docs/verification.md](verification.md) |
| `pnpm measure` | klat-hook size and start time, test counts, edit-test loop time: the refactor baseline |
| `pnpm lint` / `pnpm typecheck` / `pnpm test` | ESLint / `tsc --noEmit` / Vitest |
| `pnpm lint:rust` / `pnpm test:rust` | `cargo fmt --check` + `clippy -D warnings` / `cargo test --workspace` |
| `cargo build -p klat-hook` | Debug build of the hook (`target/debug/klat-hook.exe`, next to the dev app) |
| `.\scripts\send-test-event.ps1 <Hook> [sessionId]` | Simulate a Claude Code hook through the real adapter (`scripts/send-test-event.sh` on macOS) |
| `pnpm check:mac` | Type-check (clippy `-D warnings`) the workspace for macOS. From Windows it uses `scripts/fake-cc` in place of Apple's clang, so it proves the Rust compiles but not that it runs |

`pnpm tauri build` without the release config builds the app without the bundled hook. Use `pnpm release`, which adds `src-tauri/tauri.release.conf.json` (sidecar plus `build:hook`).

### Publishing a release

The GitHub workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml) builds the Windows installer and the universal macOS `.dmg` on GitHub's runners, so no Mac is needed.

1. Set the new version in `src-tauri/tauri.conf.json` (and `package.json`, the `Cargo.toml`s), commit and push.
2. Tag it and push the tag: `git tag v0.1.0`, `git push origin v0.1.0`. The tag must be `v` plus that version, or the workflow stops.
3. When the run under **Actions → Release** is green, **Releases** has a draft `v0.1.0` with `Klat_0.1.0_x64-setup.exe`, `Klat_0.1.0_universal.dmg` and one `SHA256SUMS.txt`. Edit the notes and click **Publish release**.

**Actions → Release → Run workflow** (no tag) builds both and leaves them as the run's artifacts, without a release.

### Signing a release

Releases are not signed yet; `SHA256SUMS.txt` is the stopgap. Once certificates exist, Tauri signs during `pnpm release` without code changes:

- Windows: add the certificate to the build machine's store and pass its thumbprint, e.g. `pnpm tauri build --config src-tauri/tauri.release.conf.json --config '{"bundle":{"windows":{"certificateThumbprint":"<thumbprint>","digestAlgorithm":"sha256","timestampUrl":"http://timestamp.digicert.com"}}}'` (or `bundle.windows.signCommand` for a cloud HSM).
- macOS: set `APPLE_SIGNING_IDENTITY` (a "Developer ID Application" identity; it overrides the ad-hoc `"-"` in `tauri.macos.conf.json`) and, for notarization, `APPLE_ID`, `APPLE_PASSWORD` (an app-specific password) and `APPLE_TEAM_ID`. Then drop the "Open Anyway" and quarantine steps from [docs/macos-install.md](macos-install.md).

### Dev page (no Tauri needed)

Run `pnpm dev` and open http://localhost:1420/dev.html.

- The port runs on the left at the overlay's size (360×330) on a checkerboard; the prototype runs on the right for comparison (it needs network access for three.js).
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
| `src/bubble/ReadyButton.ts` | The Ready button (an Aladdin lamp: Ready off sucks the cat into it) |
| `src/assets/fonts/` | Grandstander (headings, the cat's bubble) and Nunito (UI text), bundled woff2 with their OFL licences |
| `src/i18n/{en,vi}.json`, `src/i18n/index.ts` | Platform Language texts, shared with Rust; `t(lang, key, vars)` |
| `src/phrases/` | Phrase data and `PhrasePicker` |
| `src/bridge/tauriBridge.ts` | Typed events and commands |
| `src/overlay/main.ts`, `dev.ts` | Overlay entry, dev page entry |
| `src-tauri/src/` | The app shell, laid out as in [docs/ARCHITECTURE.md](ARCHITECTURE.md) §3: `bootstrap/` (builder, setup), `ipc/` (commands by domain, `events.rs` with every `cat://` name), `services/` (`cat_service.rs` the one owner of the cat's state and the settings, publishing on `watch` channels; `settings_service.rs` loading and start at login; `hooks_service.rs` hook messages), `ui/` (`overlay.rs` the transparent, topmost, never-activating window with DPI-correct placement; `tray.rs`; `settings_window.rs`), `tasks/` (`cursor_poller.rs` click-through and `cat://cursor`, `drag_loop.rs`, `wait_ticker.rs` (answered prompts, stale waits)), `platform/` (all OS code: `windows.rs`, `macos.rs`, `fallback.rs`, `webview_memory.rs`) |
| `crates/klat-core/src/i18n.rs` | Reads the shared `src/i18n` catalogs (the app takes the language from the settings) |
| `crates/klat-core/` | The rules, pure (no Tauri, no I/O, no clock): `session` (hook mapping, per-session state), `cat` (when the cat is awake), `geometry`, `settings` model, `agent_hooks` edits (Claude Code and Codex), `i18n` |
| `crates/klat-infra/` | I/O without Tauri: `event_server` (loopback HTTP endpoint, token), `runtime_file` (`runtime.json`), `hook_files` (install / repair / uninstall in `~/.claude/settings.json` and `~/.codex/hooks.json`), `settings_repo` (settings load/save), `log` (capped local log behind the `log` facade) |
| `crates/klat-hook/` | Hook adapter |
| `scripts/` | `send-test-event.ps1`, `stage-sidecar.mjs` |
| `docs/` | Prototype, [hooks troubleshooting](hooks-troubleshooting.md), [verification log](verification.md) |

## Decisions log

Small decisions made during implementation, as the spec asks (§11).

- **Phase 0**
  - Pinned versions: Tauri 2.12 (crate `tauri` 2.12.2, CLI 2.12.1), Vite 8.3.4, Three.js 0.186.1, TypeScript 6.0.3, Vitest 4.1.11, ESLint 10.
    - TypeScript 7 is not supported by `typescript-eslint` yet (it needs a version below 6.1).
    - Vitest 5 needs Node 22+, and this machine runs Node 20.19.
  - The overlay window is created in Rust (`src-tauri/src/ui/overlay.rs`) rather than declared in `tauri.conf.json`. This way it is placed at the work-area corner and created with `focused(false)`, so it never needs a `show()` call that could steal focus.
  - A minimal tray with only **Quit** is already included, because the overlay has no taskbar button and no decorations. The full tray menu (§2.3) comes in Phase 2.
  - The app icon is a generated placeholder cat head (`src-tauri/icons/`).
  - The bundle identifier is `io.github.meow-knight.klat` (`vn.ema.klat` until 2026-10-17).
  - The spike uses `THREE.Timer` instead of `THREE.Clock`, which is deprecated in three r186.

- **Phase 1**
  - `maxYellsPerWake` counts every yell of a wake, including the first "Wake up" yell. With the default of 10, the cat yells the wake phrase and then 9 more times before it sits silent.
  - `yellNow()` is the "another session became pending" path. It yells immediately even after the cap is reached, wakes the cat if it is asleep, and restarts the ignore timer so two yells never land back to back.
  - A yell's 1.1s "settle into the squint" timer is replaced by the next yell, so it cannot close the new yell's mouth early (the prototype could).
  - `sleep()` clears the happy face and puff. In the prototype, a pet right before sleep left the ^ ^ eyes on forever, because sleep cancelled the timer that resets them.
  - On wake the cat first looks at the bubble (`lx=-.35, ly=-.08`, §6.4) until cursor data arrives.
  - The first yell follows Klat Language: "Wake up, B**CH!!!" (SFW: "Wake up, human!!!") in English, "Dậy dậy dậy, dầu nhớt đây!!!" in Vietnamese, and one of the two at random on Auto. If no wake phrase matches, it still yells with whatever wake phrase is left; if the filters leave the later pool empty, the picker falls back to the wake phrase.
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
  - Settings use field-level fallback: a mistyped or out-of-range field falls back to its own default and the others are kept. Only a file that is not a JSON object counts as corrupt. Unknown fields are dropped on the next save. Numbers outside their range are clamped into it: `yellIntervalMs` 5000 to 600000, `maxYellsPerWake` 1 to 50, `staleAfterMs` 300000 to 14400000. `uiLanguage` is `en`/`vi` and `klatLanguage` is `en`/`vi`/`auto`; a file from before these existed has its `languages` list converted (only `en` → `en`, only `vi` → `vi`, anything else → `auto`). The only supported corner is `bottom-right`.
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
  - Rust is now a Cargo workspace (`src-tauri`, `crates/klat-hook`) with one `Cargo.lock` and one `target/` at the root.
  - The event mapping was checked against the official hooks docs (2026-10). It differs from the §8.1 sketch in these ways:
    - `Notification` is a fallback only. It is ignored when the session is already waiting. `permission_prompt` fires ~6s after `PermissionRequest`, and `idle_prompt` ~60s after `Stop`, so neither re-wakes a cat you already dismissed.
    - Besides `idle_prompt`, `agent_needs_input`, `elicitation_dialog` and `elicitation_url_dialog` also mean "needs input".
    - `StopFailure` (a turn that ended on an API error) counts as done. `PostToolUseFailure` clears a permission wait like `PostToolUse` does, because the tool ran, so you approved it.
    - `SubagentStop` is ignored, since the main agent is still working.
    - `PermissionDenied` only fires in auto mode. A manual No/Esc is a user interrupt, and `Stop` does not run after one (corrected 2026-10-21; this note used to say `Stop` follows).
  - The 2s dedupe applies to the same session and the same reason. A permission wait is cleared only by the tool that asked for it, or by any tool when the wait came from a Notification fallback with no tool name.
  - No hook runs when you answer a permission prompt (checked in the docs and in Claude Code 2.1.306: `PermissionRequest` hooks run alongside the dialog and are not told the answer). Yes shows only as `PostToolUse` when the tool finishes; No/Esc shows as nothing. Two later additions (ClauCat-377):
    - `PostToolBatch` (not from a subagent: `klat-hook` forwards only whether `agent_id` is present) clears a permission wait whatever the tool, which covers No with a message for Claude, after which Claude goes on with other tools.
    - Claude Code sends `Notification(permission_prompt)` only for a prompt left unanswered ~6s (each keystroke defers it). A Claude Code `PermissionRequest` with no such notification within 8s (`PROMPT_ANSWERED_AFTER`) is cleared as answered, by the `wait_ticker` (1s). A notification that arrives after that wakes the cat again through the normal fallback rule. Codex is left out: it has no Notification hook.
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
  - **Start at login** uses `tauri-plugin-autostart`, which writes the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value `Klat`. It is off unless you tick it. At start-up the Run key is treated as the truth (you may have disabled it in Task Manager) and `startAtLogin` is synced to it.
  - **Log:** `%APPDATA%\Klat\klat.log`, rotated to `.old` at 512 KB, one line per entry, each line at most 300 characters. It records names and outcomes only: 8 characters of the session id, the project folder name, event names, HTTP status of rejected requests, installer and autostart results. In debug builds lines also go to stderr.
  - **Bundling:** `klat-hook` ships as a Tauri sidecar (`bundle.externalBin`), which the installer places next to `klat.exe` as `klat-hook.exe`. That is exactly where the hooks installer points. The sidecar config lives in `src-tauri/tauri.release.conf.json` (used by `pnpm release`), because tauri-build would otherwise require the staged binary for every dev build. In dev the workspace build puts the hook in `target/debug` next to the app.
  - The NSIS installer is per-user (`%LOCALAPPDATA%\Klat`) and needs no admin rights. Uninstalling does not touch `settings.json` hooks or `%APPDATA%\Klat`.
  - Memory (release, 185 MB private) is over the ~150 MB goal. `--in-process-gpu` saved ~10 MB but ties GPU crashes to the browser process, so WebView2 keeps its defaults. The numbers are recorded under Performance.
  - The README is reorganised for users first (install, use, hooks, uninstall, privacy, troubleshooting, limitations, performance), then development. Per-phase verification moved to [docs/verification.md](verification.md).
- **After the MVP: Ready and the aura** (user request, 2026-10-11)
  - **Ready.** A finished turn (`Stop`/`StopFailure`) no longer wakes the cat by default. The user explained the two situations: when they watch Claude, waking on every reply only distracts; when they hand Claude a long task and look away, they want the old behaviour.
    - Ready is armed with a small button next to the sleeping cat, or from the tray.
    - It counts only finished turns that arrive **after** arming. A turn that ended before you clicked does not fire it.
    - It stays armed until you turn it off (the lamp or the tray). The first version was one-shot (it switched off once the cat had woken you), but the user wanted it to stay on.
    - Permission and input waits always wake the cat, as the user chose. Pause still silences everything.
    - Ready lives in Rust (`klat_core::cat`, the time it was armed), and `session_store::Gate` decides which waits may wake the cat. It is sent to the overlay as `cat://state.ready`.
    - A finished turn that arrives without Ready is still tracked, but it neither wakes the cat nor makes an awake cat yell again.
  - **Ready feedback.** Arming makes the cat peek, perk one ear (kept up while it listens) and say a short line (`ready` phrases, en/vi, SFW-safe) for 2.2s. The button uses only static CSS: an endless CSS animation would repaint the transparent window at the display rate, as the old Zzz did.
  - The button shows only while the cat is asleep and shown. With "Hide cat while sleeping" on, use the tray item instead.
  - **Aura.** Gold Super Saiyan energy, drawn as one shader quad behind the cat (premultiplied colour with custom blending, for the transparent window).
    - Sharp spikes shoot up from an egg-shaped outline around the cat: longest above the head, shorter down the shoulders.
    - Two walls: a tall pale outer one and a bright inner one with a white-hot core.
    - Vertical streaks rush upwards, with a soft halo around the spikes.
    - Up to two white zigzag lightning arcs crackle around the body, re-rolled 9 times a second.
    - Spike heights jump at ~14 steps a second, like hand-drawn anime frames.
    - The first version (soft flames) was rejected as not sharp enough; the reference was a Super Saiyan Goku artwork.
    - Its intensity is a spring: 1 when awake, 0 asleep (the quad is hidden then), flaring past 1 on each yell and on wake.
    - Under reduceMotion it glows steadily, with no flicker and no flares.
    - It sits outside the click hitbox and costs one extra draw call while awake.
  - **Overlay 360×330.** The window grew by 50px of headroom above the cat, so the spikes can rise. The camera keeps the scale of the 280px framing (the fov grows with the height and the camera lifts), so the cat stays the same size in the same place. The bubble is anchored to the bottom (`top: calc(100% - 266px)`). The empty area is click-through, so the larger window costs nothing.
  - **Ready button** moved to the right of the sleeping cat. Its icon is a cat's collar bell (pink collar, jingle bell; gold with a shine when armed), replacing the alarm clock: "ring the bell, the cat rings you back".
  - **Double-click the awake cat** to send it back to sleep without typing ("not now"). It takes the same dismiss path as clicking the bubble. A single click still pets.
  - **Ready stays on** (2026-10-12): it no longer switches itself off after waking you. Every finished turn wakes the cat until you click the bell again.
  - **Aladdin lamp** (2026-10-22): the bell became a lamp (alarm clock → bell → lamp). Clicking it turns Ready off and sucks the cat into the lamp: the canvas shrinks and spins into the spout (`stage.css .in-lamp`, origin `--lamp-x/--lamp-y`), then the render loop stops and the cat drops out of the hitboxes, so only the lamp takes room and clicks. Clicking again turns Ready on and the cat pops out with an overshoot. `CatStage.syncLamp`: in the lamp = Ready off and asleep, so the tray's Ready item does the same, a wake (permission, input) makes the cat burst out, and after the knockout it goes back in. No Rust state: Ready is still the only switch. reduceMotion and the first state from Rust switch without the transition. The lamp glows gold with a wisp of smoke while the cat is inside.
  - **Poking the sleeping cat** (2026-10-12): instead of peeking one eye open, it keeps its eyes shut, knits its brows, an anime angry mark (red, white rim, a canvas-drawn sprite on the head) pops up on the head's top-right corner, between the ear and the brow, and throbs, and the tail swishes on top of the sleepy sway for 1.4s. Poking again restarts it; waking clears it. Under reduceMotion the mark shows without the pop, throb or swish. A first version floated beside the head on the left; the user wanted it on the head itself.
- **Knockout** (2026-10-14, owner request): when you answer or dismiss, the cat no longer just yawns. Rust sends `sleepCause` (`user` for hooks, dismiss and Ready; `auto` for stale expiry and Pause) with `cat://state`, and the overlay picks the sequence. A random object (stick, dép tổ ong, durian, non-stick pan, built from primitives in the toon style) flies in from the left (the durian drops from above), hits the top of the head at 380 ms with a comic burst, then bounces away; the cat gets X eyes, stars circling its head and a bump, slumps over at ~1 s and is settled asleep at ~2.1 s. Under reduce motion nothing flies: a moment of X eyes, then sleep. Frames of each object are in `docs/images/knockout/`.
- **More lines, more languages, more objects** (2026-10-17, owner request, ClauCat-85c): Klat Language gains 中文, 日本語 and 한국어 (`zh`, `ja`, `ko`; Auto now mixes all five). Each language has an SFW and an explicit wake line and lines for every reason; English and Vietnamese got about twenty more each. The bubble carries the phrase's `lang`, so CJK text uses the system's font for that language (Grandstander has no CJK glyphs). Six more knockout objects: a fish, a rubber duck, a coffee mug and a bánh mì are thrown; a nón lá skims in flat like a frisbee; an anvil drops from above and barely bounces. `characters.html?knockouts=0.3` shows every object at that point of its flight. As the object flies in the cat blurts out a gasp in its language ("Oh sheeet!", "Âu shịt!", 卧槽！, やっべ、クソッ！, 아 씨*!; under SFW "Oh snap!" and the like) until it slumps over dizzy at ~1 s; not under reduce motion, where nothing flies.
- **Cat size** (2026-10-14, owner request, ClauCat-gra): a slider in Settings → General, 50–200% in steps of 1, stored as `catScale`. The overlay window becomes 360×330 times the scale and its webview is zoomed by the same factor, so the page keeps laying out at 360×330 CSS px and the cat, bubble and Ready button grow together, sharp because `devicePixelRatio` includes the zoom. Rust converts between the two: hitboxes arrive in CSS px and are scaled up on read, cursor points are scaled down before they are sent. Resizing keeps the window's bottom-right corner, so the cat stays where it is; a dragged cat's new top-left is saved. While you drag, the slider ignores late replies, so the thumb never jumps under your hand. **Smooth dragging** (2026-10-15, ClauCat-jds): resizing the window on every step was jerky (size, position and zoom land separately, about 6–8 visible steps a second). Now a drag (or arrow keys) only previews: Rust sets the window once to the 200% size and the page scales the cat with a CSS transform anchored bottom-right, one step per frame; on release (keys: 400 ms after the last press) the value is saved and the window set once to the real size. The page hides the cat while each of those two switches lands (one frame in a screen capture), and the stage redraws right after a resize so the canvas is never blank. The stage now also follows `devicePixelRatio` changes (zoom, or a monitor with another DPI) instead of only CSS resizes.
- **Memory** (2026-10-14, ClauCat-gwd): the 150 MB goal is met for what Task Manager shows, not for committed memory. The network service moved into the browser process, and the overlay asks WebView2 for its low memory target while the cat sleeps. Both webviews pass the same `BROWSER_ARGS` (wry's defaults plus the feature flag): WebView2 refuses a second webview in the same data folder with different arguments. Software rendering would get committed memory under 150 MB but was rejected for its CPU cost. Numbers under [Performance](#performance).
- **Security hardening** (2026-10-16, ClauCat-fw3, after a security audit):
  - pnpm is pinned to 10.34.6 with its sha512 (`packageManager`); pnpm 8.13.1 had 26 advisories, 12 of them high (install-time path traversal, lockfile integrity bypass, command injection). The lockfile moved to v9 with every resolved version unchanged.
  - The event server gives each request 500ms in total (the adapter's own budget is 300ms), so a client dripping a byte at a time can no longer hold one of the 16 slots for hours. Rejected requests are logged 5 per minute at most, then counted, so a flood cannot rotate real entries out of the capped log.
  - `runtime.json` (the token) is never written to the temp folder: without `APPDATA` / `HOME` the hook events are off. It is created fresh (a planted temp file or link is not followed) and is 0600 on Unix.
  - Hook entries are ours only when the program's file name is `klat-hook` / `claude-cat-hook`; a command that merely contains the name is someone else's. Only the newest three `settings.json.bak.*` are kept, since the file can hold secrets.
  - The session map forgets sessions quiet for `staleAfterMs` and holds 256 at most.
  - Release builds on Windows clear `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (and the browser-folder and script-debugger variables) at start, so no other program can open DevTools on the app. Debug builds keep them for driving the app in checks.
  - Not code-signed yet (needs a Windows code-signing certificate and an Apple Developer ID). Until then every release publishes `SHA256SUMS.txt`, and the macOS guide only removes the quarantine mark after the checksum matched.
  - **Per-user install kept** (owner decision): `klat-hook.exe` sits in `%LOCALAPPDATA%\Klat`, which the user can write, and Claude Code runs it on every hook with the hook's full input. Malware already running as the user could replace it. A per-machine install would close this but needs admin rights to install and update; the owner chose to keep installs admin-free.
  - Not applicable: RustSec advisories for `glib` 0.18.5 (RUSTSEC-2024-0429) and `proc-macro-error` (RUSTSEC-2024-0370, unmaintained) come only through Tauri's GTK stack on Linux, which Klat does not build; they are ignored in `.cargo/audit.toml`.
- **Drag and drop** (2026-10-12): hold Space, click the cat and drag; releasing Space drops it.
  - The overlay never takes focus, so it cannot see the keyboard. A low-level keyboard hook (`platform/windows.rs`, `macos.rs`) looks at Space only, and only takes it from the focused app while the pointer is over the cat (or its bubble or Ready button) with no modifier held. Anywhere else Space passes through untouched. A tap with no click is replayed to the focused app with `SendInput`, so typing with the pointer resting on the cat is not lost.
  - Grab plus the primary mouse button starts the drag. The window follows the cursor at ~120Hz until Space is released. The grip is kept in logical px, so crossing to a monitor with another DPI keeps the cat in hand, and the DPI re-place is skipped while dragging.
  - The cat (not the whole window, which is mostly transparent) is kept inside the work area of the monitor under the cursor, so it can sit flush against any edge.
  - The spot is saved as `position` in settings.json (window top-left, physical px). At start it is used only if the window's centre is still on a monitor; otherwise the cat goes back to the corner. (A tray item "Move the cat back to the corner" existed until 2026-10-13; it was dropped because you can drag the cat.)
  - The press that moves the cat is swallowed in the capture phase (`cat://grab`), so it never pets, dismisses or toggles Ready. Click-through is never turned on under a cat that is being dragged.
