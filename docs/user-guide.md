# Klat user guide

Everything about using Klat: what the cat does, the tray and Settings, the hooks it installs, uninstalling, privacy and troubleshooting. The short version is in the [README](../README.md).

## Install

0. Klat is not code-signed yet, so Windows SmartScreen warns about the installer. Check the download first: in PowerShell, `Get-FileHash Klat_<version>_x64-setup.exe -Algorithm SHA256` must print the same hash as `SHA256SUMS.txt` from the release. If it differs, do not run it.
1. Run `Klat_<version>_x64-setup.exe`. It is a per-user install into `%LOCALAPPDATA%\Klat`, so no admin rights are needed. The installer contains `klat.exe` and the hook program `klat-hook.exe`.
2. Start **Klat**. A sleeping cat appears in the bottom-right corner, and a cat icon appears in the tray (it may be under the **^** hidden-icons arrow).
3. Tray → **Settings…** → **Hooks**. There is one card per agent: **Install** under **Claude Code** adds Klat's hooks to `%USERPROFILE%\.claude\settings.json`, **Install** under **Codex** adds them to `%USERPROFILE%\.codex\hooks.json` (or `%CODEX_HOME%\hooks.json`). Install one, the other or both. A backup is saved first, and your other hooks and settings stay as they are.
4. Codex only: open Codex, type `/hooks` and trust Klat's hooks. Codex skips hooks you have not reviewed, and asks again after **Repair** (the entries change).
5. Restart any open Claude Code or Codex sessions. From now on the cat wakes when Claude asks for permission or input, and when Claude finishes a turn: Ready is on when Klat starts. Click the **Aladdin lamp** to the right of the cat to turn it off if you only want to be woken for permission and input: the cat is sucked into the lamp, so only the lamp takes room on your screen.

You need Windows 10/11 with the WebView2 runtime, which comes with Windows 11 and recent Windows 10.

### macOS

Klat builds for macOS 11+ (a menu bar app, no Dock icon). It has not been run on a real Mac yet. Install a
`.dmg` with [docs/macos-install.md](macos-install.md) (it is not notarized, so macOS asks you to confirm
once), or build from source with [docs/macos-build.md](macos-build.md), which also shows how to build the
`.dmg`. Dragging the cat with Space needs the Accessibility permission; Settings → General asks for it.

## Using it

| When | The cat |
|---|---|
| Claude asks for permission or waits for input | Jumps up with a gold aura and yells, then glares and taps its paw; it yells again every 30s (10 times at most) |
| Claude finishes a turn | Wakes you as above while Ready is on (the cat is out of the lamp); keeps sleeping in the lamp while it is off |
| You click the lamp while the cat is out | Ready turns off: the cat spins into the lamp's spout and is gone. The lamp glows gold with a wisp of smoke, and only the lamp is clickable |
| You click the lamp while the cat is inside | Ready turns on: the cat pops out, perks one ear and says "Ok, Claude xong tao gọi" (or similar), then sleeps on. Ready stays on, for every turn that finishes, until you click the lamp again |
| Claude needs you while the cat is in the lamp | It bursts out of the lamp and yells as above. Once it is knocked out, it is sucked back in |
| More than one session needs you | Yells again for each new one and names the project under the phrase |
| You send a prompt, answer a permission prompt, or the session ends | Gets knocked out: one of ten objects (a stick, a honeycomb slipper (dép tổ ong), a durian, a frying pan, a fish, a rubber duck, a coffee mug, a bánh mì, a nón lá or an anvil; picked at random, never the same twice in a row) flies in and bonks it on the head; X eyes, stars and a bump, then it slumps asleep. Ready stays on |
| You click the bubble, or double-click the awake cat | Knocked out the same way ("not now"). This marks those sessions as seen and sends nothing to Claude Code |
| A waiting session expires, or you turn on Pause | Nobody did anything, so it just yawns and goes back to sleep |
| You click the cat | Asleep: keeps its eyes shut, frowns, an angry mark pops up on the top-right corner of its head and its tail swishes, then it calms down. Awake: purrs (^ ^) |
| You move the mouse while it is awake | Follows the cursor anywhere on the main screen |
| You hold **Space**, click the cat and drag | The cat follows the mouse (the cursor turns into a hand). Release Space to drop it there; the spot is remembered after a restart |

The overlay never takes focus, and clicks on its empty area go through to the window below.

**Tray menu** (quick actions only):

| Item | What it does |
|---|---|
| Ready: wake me when Claude is done | Same as the lamp (ticked = the cat is out); the two stay in sync |
| Pause notifications | The cat stays asleep. Sessions are still tracked, and unpausing wakes it if something is still waiting |
| Dismiss all | Puts the cat to sleep and marks waiting sessions as seen |
| Settings… | Opens the Settings window (on Windows, double-clicking the tray icon does too) |
| Quit | Quits the app |

The tray tooltip shows how many sessions are waiting.

**Settings window:**

| Section | What is in it |
|---|---|
| General | Start at login (off by default; an entry in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), Character (cat, snake, dragon, capybara or slime, each with a one-line description of its temper), Pause notifications, Hide the cat while it sleeps, SFW mode (no swearing; the English first line becomes "Wake up, human!!!", the Vietnamese one is already clean), Reduce motion, Cat size (a slider, 50–200%), and how to move the cat |
| Language | Platform Language (menus, dialogs, this window) and Klat Language (what the cat says: English, Tiếng Việt, 中文, 日本語, 한국어, or Auto for all of them), with a sample line |
| Hooks | For Claude Code and for Codex, each on its own: whether it is wired to Klat, the file it uses (`settings.json`, `hooks.json`), and Install / Repair / Uninstall (see [Hooks](#hooks)) |
| Advanced | Time between yells; Wake test and Sleep buttons to try the cat out |

Changes apply at once; there is no Save button.

**Settings** are in `%APPDATA%\Klat\settings.json`. The Settings window writes the file immediately. Hand edits are read at start-up. A missing or invalid field falls back to its default. A file that is not JSON at all is renamed to `settings.json.corrupt.<time>` and replaced with defaults.

| Field | Default | |
|---|---|---|
| `notificationsEnabled` | `true` | `false` = paused |
| `hideWhileSleeping` | `false` | |
| `sfwMode` | `false` | |
| `uiLanguage` | `"en"` | Platform Language for the tray, dialogs and the Settings window: `en` or `vi` |
| `klatLanguage` | `"auto"` | What the cat says: `en`, `vi`, `zh`, `ja`, `ko`, or `auto` (every language). Older files with `languages` are converted on load |
| `yellIntervalMs` | `30000` | 5000 to 600000 |
| `maxYellsPerWake` | `10` | Includes the first yell; 1 to 50 |
| `staleAfterMs` | `1800000` | A session waiting longer than this stops counting; 300000 to 14400000 |
| `reduceMotion` | `false` | No jumps, squash, puff, text shake, purr shake or Zzz; the aura glows steadily without flickering or flaring; springs are critically damped |
| `catScale` | `100` | Cat size in percent, 50 to 200: the cat, its bubble and the Ready button together |
| `startAtLogin` | `false` | Use the Settings window; it keeps the registry in sync |
| `corner` | `"bottom-right"` | The only corner supported so far |

## Hooks

**Hooks → Install** adds one entry per event (`Stop`, `StopFailure`, `PermissionRequest`, `Notification`, `UserPromptSubmit`, `PostToolUse`, `PostToolUseFailure`, `PostToolBatch`, `PermissionDenied`, `SessionEnd`) to `%USERPROFILE%\.claude\settings.json`. Each entry looks like this:

```json
{ "type": "command", "command": "C:\\Users\\you\\AppData\\Local\\Klat\\klat-hook.exe", "args": ["Stop"], "timeout": 1 }
```

- It runs the program directly (exec form), with no shell involved, so paths with spaces are fine.

**Hooks → Codex → Install** adds one entry per event Codex has (`Stop`, `PermissionRequest`, `UserPromptSubmit`, `PostToolUse`, `SessionEnd`, `Interrupt`) to `%USERPROFILE%\.codex\hooks.json` (`%CODEX_HOME%\hooks.json` when that is set). Codex has no exec form: it runs the `command` line through `cmd.exe /C` on Windows (`$SHELL -lc` on macOS). On Windows the program is written without quotes, because Codex 0.144 escapes quotes in a way `cmd.exe` cannot read; a folder with spaces is written with its short 8.3 name (`C:\PROGRA~1\...`) instead:

```json
{ "type": "command", "command": "C:\\Users\\you\\AppData\\Local\\Klat\\klat-hook.exe --agent codex Stop", "timeout": 2 }
```

- Codex runs a hook only after you trust it: type `/hooks` in Codex and trust Klat's entries. Trust is tied to the entry's exact text, so after **Repair** Codex asks again.
- Codex has no `Notification` event, so with Codex the cat wakes when a turn ends or Codex asks for permission, not when it idles waiting for input. `Interrupt` (you stopped the turn) clears the wait.

For both agents:

- Before any change, the file is saved as `<file>.bak.<unix-seconds>` (the newest three are kept).
- If the file is not valid JSON, nothing is written and the Settings window says so.
- Installing again changes nothing. **Repair** re-points the entries after you move or reinstall the app, or switch between a dev build and the installed one.
- **Uninstall** removes only entries whose program is `klat-hook` (`klat-hook.exe` on Windows), and keeps everything else, including key order.
- Project-level files (`.claude/` or `.codex/` inside a repository) are never touched.
- The app never installs or changes hooks on its own; only the buttons in Settings → Hooks do.

The hook reads the event from stdin and forwards five fields: event name, session id, project folder name, tool name and notification type, plus which agent ran it (Codex entries pass `--agent codex`). It sends them to `127.0.0.1`, prints nothing, and always exits 0. Its budget is 300ms. When the app is not running, it exits in a few milliseconds.

## Uninstall

1. Tray → **Settings…** → **Hooks** → **Uninstall** (under each agent you installed for), then tray → **Quit**.
2. Windows Settings → Apps → **Klat** → Uninstall.
3. Optional: delete `%APPDATA%\Klat` (settings and log) and any `%USERPROFILE%\.claude\settings.json.bak.*` or `%USERPROFILE%\.codex\hooks.json.bak.*` backups you no longer need.

If you uninstall the app first, the hook entries stay in `settings.json` and `hooks.json`. They are harmless (the program is gone, so the hook just fails quietly), but you can delete them by hand: they are the entries whose `command` contains `klat-hook`.

## Privacy

- **Nothing leaves your machine.** The app listens only on `127.0.0.1`, on a random port, with a fresh 256-bit token on each run. `%APPDATA%\Klat\runtime.json` holds the port and token while the app runs and is deleted when you quit.
- The server rejects anything without the token, any request with an `Origin` header (web pages), bodies over 64 KB, and unknown event names.
- The hook never sends prompts, commands, tool input or output, messages, transcript paths or full folder paths. It sends only the five fields listed under [Hooks](#hooks).
- The local log (`%APPDATA%\Klat\klat.log`, capped at 512 KB plus one `.old` file) records start-up, hook event names with the first 8 characters of the session id and the project folder name, rejected requests, and installer and autostart actions.
- The hook never makes permission decisions.

## Troubleshooting

| Problem | Try |
|---|---|
| The cat doesn't wake when Claude finishes | Check that Ready is on: the cat must be out of the lamp (click the lamp to let it out). If it still doesn't wake, open Settings → **Hooks**: does it say **Installed**? If not, click Install or Repair. Then restart the Claude Code session. See [docs/hooks-troubleshooting.md](hooks-troubleshooting.md) for step-by-step checks and how to read the log |
| The cat doesn't wake for Codex | Settings → **Hooks** → **Codex** must say **Installed**. Then in Codex type `/hooks`: Klat's entries must be trusted (after Repair, trust them again). Check that `[features] hooks = false` is not in `~/.codex/config.toml`. Start a new Codex session |
| "Hooks (needs repair)" | The entries point at another copy of `klat-hook.exe` (moved or reinstalled app, dev vs installed): run **Repair** |
| Install says settings.json (or hooks.json) is invalid | Fix the JSON, or restore a `.bak.*` backup; nothing was changed |
| The cat stays awake | Send a prompt or click the bubble. A session waiting longer than 30 minutes is dropped automatically |
| Opening the app again shows "already running" | Only one cat runs at a time. Look in the tray |
| Black or white box instead of a transparent window | Update the WebView2 runtime and your GPU driver |

## Known limitations

- Space only starts a drag while the pointer is over the cat, and only without Shift/Ctrl/Alt/Win. A plain tap of Space there is given back to the app you are typing in, but holding Space there (auto-repeat) types a single space.
- Windows only. The cat lives in the bottom-right corner of the **primary** monitor's work area. With the cursor on another monitor, the cat looks at its bubble instead of following.
- Tested at 100% and 125% display scaling. 150% and multi-monitor setups are covered only by unit tests so far.
- **Memory:** Task Manager shows ~25–50 MB while the cat sleeps, but the memory WebView2 has committed stays at ~175 MB (6 processes; ~90 MB of it is the GPU process). Getting that under the spec's ~150 MB would mean software rendering, which costs 5× the CPU (see [Performance](development.md#performance)).
- No Claude Code hook runs at the moment you answer a permission prompt. Klat counts a prompt as answered when Claude Code has not reported it unanswered within ~6s (the cat falls asleep ~8s after it woke), or when the tool finishes, the batch of tool calls resolves, or you send a prompt. So if you answer **after** those ~6s, Yes on a long command keeps the cat awake until the command finishes, and No/Esc keeps it awake until your next prompt (or click the cat to dismiss it). With Codex (no Notification event), only the tool finishing or your next prompt counts.
- `claude -p` (print mode) ends its session at once, so the cat wakes and falls asleep again within milliseconds.
- Right after you use the tray menu, clicking the bubble can give focus to the overlay (Windows hands focus over from the closing tray window). With your terminal focused, it never takes focus.
- The hitbox is a generous box around the cat rather than its exact outline, so clicks just next to the cat can land on it.
- Ready is one global switch, not per session. Any session that finishes after you arm it wakes the cat. Ready is not saved: it is on at every launch, even if you turned it off before quitting.
- The Windows uninstaller does not remove the hooks from `settings.json`; use **Hooks → Uninstall** first.
