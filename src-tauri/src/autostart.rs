//! Start at login (spec §10 `startAtLogin`): opt-in only. An HKCU Run entry on Windows, a LaunchAgent on macOS.

use crate::log;
use crate::state::{self, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

/// Whether the OS entry exists. It is the truth: the user may have removed it (Task Manager, Login Items).
pub fn is_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// At start: the OS entry is the truth (the user may have removed it in Task Manager), so the setting follows it.
pub fn reconcile(app: &AppHandle) {
    let on = is_enabled(app);
    if on != app.state::<AppState>().settings().start_at_login {
        state::update_settings(app, |s| s.start_at_login = on);
    }
}

/// Adds or removes the OS entry, then records the choice in settings. On failure settings are left alone.
pub fn set(app: &AppHandle, on: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    match result {
        Ok(()) => {
            log::info(format!(
                "start at login {}",
                if on { "enabled" } else { "disabled" }
            ));
            state::update_settings(app, |s| s.start_at_login = on);
            Ok(())
        }
        Err(e) => {
            log::warn(format!("could not change start at login: {e}"));
            Err(e.to_string())
        }
    }
}
