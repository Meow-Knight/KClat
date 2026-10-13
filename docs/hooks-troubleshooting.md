# Hooks troubleshooting

Klat learns about Claude Code through hooks in `%USERPROFILE%\.claude\settings.json`. Each hook runs `klat-hook.exe`, which forwards a few fields to the app over `127.0.0.1` and exits. When something does not work, go through the steps below in order.

## 1. Is the app running?

Look for the cat in the tray (it may be under the **^** hidden-icons arrow). Then check for `%APPDATA%\Klat\runtime.json`:

- The app writes this file when it starts and deletes it when you quit.
- If the app crashed or was killed, a stale copy can stay behind. It is harmless: the next start overwrites it, and the hook gives up on a dead port in ~150ms.

## 2. Are the hooks installed?

Tray → **Settings…** → **Hooks** shows the status (the dot next to **Hooks** in the sidebar too):

| Status | Meaning | What to do |
|---|---|---|
| Installed | All entries are present and point at this app's `klat-hook.exe` | Nothing |
| Needs repair | Some entries are missing, duplicated, or point at another path (for example, you moved or reinstalled the app, or switched between a dev build and the installed one) | **Repair** |
| Not installed | No Klat entries | **Install** |

To check by hand, open `%USERPROFILE%\.claude\settings.json` and look under `"hooks"` for entries like this:

```json
{ "type": "command", "command": "C:\\Users\\you\\AppData\\Local\\Klat\\klat-hook.exe", "args": ["Stop"], "timeout": 1 }
```

There should be one for each of `Stop`, `StopFailure`, `PermissionRequest`, `Notification`, `UserPromptSubmit`, `PostToolUse`, `PostToolUseFailure`, `PermissionDenied` and `SessionEnd`.

## 3. Has Claude Code picked them up?

New Claude Code sessions read the hooks when they start. If a session was already open when you installed, restart it. Inside Claude Code, `/hooks` lists the active hooks.

## 4. Does the path work?

Run the hook by hand. It must print nothing and exit with code 0:

```powershell
'{"session_id":"manual","cwd":"C:\\x\\demo","hook_event_name":"Stop"}' | & "<path from settings.json>"
$LASTEXITCODE   # 0
```

With the app running, the cat should wake up. From a clone of this repo, `.\scripts\send-test-event.ps1 Stop` does the same.

## 5. Read the log

`%APPDATA%\Klat\klat.log` (capped at 512 KB, plus one `.old` file) records start-up, every hook event the app received (event name, the first 8 characters of the session id, the project folder name, and what it did), rejected requests, and installer actions.

| Log line | Meaning |
|---|---|
| `hook Stop session 1a2b3c4d project "repo" -> attention Done` | Working |
| `-> ignored` | Expected for duplicates within 2s, Notification fallbacks for a session that is already waiting, and tool events that don't match a permission wait |
| `rejected a request ... HTTP 401` | A hook used an old token. Start the app before Claude Code sends events, or make sure only one copy of the app runs |
| No `hook` lines at all | Claude Code is not running the hook: go back to steps 2–4 |

## 6. Broken `settings.json`

If `settings.json` is not valid JSON, **Install / Repair / Uninstall refuse to write** and say so in a message box. Fix the file, or restore one of the `settings.json.bak.<timestamp>` backups that the installer makes before every change, then try again.

## 7. Uninstalling

Run Settings → **Hooks** → **Uninstall** before removing the app. It removes only Klat's entries and keeps everything else in `settings.json`, including other hooks and the order of keys. If the app is already gone, delete the entries whose `command` contains `klat-hook.exe` by hand. While the app is not running, leftover entries only cost a few milliseconds per hook.

## What the hook never does

- It never prints to stdout and always exits 0, so Claude Code can never read its output as an allow or deny decision.
- It never sends tool input, commands, prompts, messages or transcript paths. It sends only the event name, session id, project folder name, tool name and notification type.
- It never talks to anything except `127.0.0.1`.
