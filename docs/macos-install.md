# Installing Klat on a Mac

For people who just want the cat. You need macOS 11 (Big Sur) or later, on Apple Silicon or Intel: the same
`.dmg` works on both.

> **Status:** the `.dmg` has not been built and installed on a real Mac yet (ClauCat-t0b.14). The steps below
> are what macOS does with an app like Klat; the screenshots get added after that first install.

## Why macOS warns you

Klat is not signed with a paid Apple Developer ID and has not been checked ("notarized") by Apple. That does
not mean anything is wrong with it, only that macOS cannot vouch for it, so the first time you open it macOS
stops it and asks you to confirm. You only do this once per install.

## 1. Copy Klat to Applications

1. Open `Klat_<version>_universal.dmg`.
2. Drag **Klat** onto the **Applications** folder in the window that opens.
3. Eject the disk image (the ⏏ button next to "Klat" in Finder's sidebar).

<!-- screenshot: docs/images/macos/01-dmg-window.png -->

## 2. Open it the first time

1. Open **Applications** and double-click **Klat**. macOS says it cannot verify Klat and offers only
   **Done** or **Cancel** (and **Move to Trash**). Click **Done** / **Cancel**.
2. Open **System Settings → Privacy & Security** and scroll down to **Security**. There is a line saying
   "Klat" was blocked, with an **Open Anyway** button. Click it, confirm with your password or Touch ID, and
   click **Open** in the next prompt.

<!-- screenshot: docs/images/macos/02-blocked.png -->
<!-- screenshot: docs/images/macos/03-open-anyway.png -->

Klat starts: a cat icon appears in the **menu bar** (top right) and a sleeping cat in the bottom-right corner
of the screen. There is no Dock icon; Klat lives in the menu bar.

### If macOS says Klat "is damaged and can't be opened"

Klat is not damaged. When a file is downloaded, macOS marks it as "from the internet", and for apps without a
Developer ID it sometimes words its refusal this way. Remove that mark and open Klat again:

1. Open **Terminal** (Applications → Utilities → Terminal).
2. Copy this line, paste it into Terminal and press Return:

   ```bash
   xattr -cr /Applications/Klat.app
   ```

   It prints nothing when it works. It only changes Klat, nothing else on your Mac.
3. Open Klat again from Applications.

## 3. Let Klat see the Space key (to drag the cat)

You can move the cat by holding **Space**, pressing the mouse on the cat and moving it. For that Klat needs
the **Accessibility** permission. Everything else works without it.

1. Menu bar cat → **Settings…** → **General**. Under *Moving the cat* click **Allow…**.
2. macOS opens **System Settings → Privacy & Security → Accessibility**. Turn on **Klat**.
3. Within about 2 seconds Settings shows **Allowed**. No restart needed.

<!-- screenshot: docs/images/macos/04-accessibility.png -->

## 4. Connect Klat to Claude Code

Settings → **Hooks** → **Install**. Klat adds its entries to `~/.claude/settings.json` (it saves a backup
first) and leaves everything else in that file alone. Restart any Claude Code sessions that are already open.

<!-- screenshot: docs/images/macos/05-hooks.png -->

To try it: Settings → **Advanced** → **Wake test**. The cat wakes up and yells; click its speech bubble to
put it back to sleep.

If you ran Klat from source before, the hooks point at that copy: click **Repair** so they point at
`/Applications/Klat.app`.

## Updating

Quit Klat (menu bar cat → **Quit**), then repeat steps 1 and 2 with the new `.dmg` and choose **Replace**.

**After an update, dragging may stop working** even though Accessibility still shows Klat turned on. Each
build has a new signature, and macOS keeps the permission for the old one. Fix it in **System Settings →
Privacy & Security → Accessibility**: select **Klat**, remove it with **−**, then go back to Klat's Settings
→ General → **Allow…** and turn it on again.

The hooks keep working across updates as long as Klat stays in Applications.

## Uninstalling

1. Settings → **Hooks** → **Uninstall** (removes only Klat's entries from `~/.claude/settings.json`).
2. Quit Klat and drag it from Applications to the Trash.
3. Optional: delete `~/Library/Application Support/Klat` (settings and log) and remove Klat from
   Accessibility.

## Screenshots to take (first `.dmg` install, ClauCat-t0b.14)

Save them under `docs/images/macos/` with these names, then replace the `screenshot:` comments above with
images. Real screenshots only.

| File | What |
|---|---|
| `01-dmg-window.png` | The opened `.dmg`: Klat and the Applications folder |
| `02-blocked.png` | The first-open warning |
| `03-open-anyway.png` | Privacy & Security with the **Open Anyway** button |
| `04-accessibility.png` | Accessibility with Klat turned on, and Settings → General showing **Allowed** |
| `05-hooks.png` | Settings → Hooks after **Install** |
| `06-cat.png` | The cat awake on the desktop, with the menu bar icon visible |
