//! The settings around the model: where the file is, loading it at start, and start at login (spec §10
//! `startAtLogin`), whose truth is the OS entry. Changing settings is `CatService::update_settings`: it shares the
//! cat's lock, so Pause and the cat's sleep are one step.

use crate::services::cat_service::CatService;
use klat_core::settings::Settings;
use klat_infra::settings_repo;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// The data folder (settings, runtime.json, log): see klat_protocol::data_dir. The temp folder if the base is
/// unknown.
pub fn data_dir() -> PathBuf {
    klat_protocol::data_dir().unwrap_or_else(|| std::env::temp_dir().join("Klat"))
}

/// Reads `settings.json` in `dir` (never fails: see settings_repo). Returns the settings and the file's path.
pub fn load(dir: &Path) -> (Settings, PathBuf) {
    let path = dir.join("settings.json");
    let (settings, outcome) = settings_repo::load(&path);
    if let settings_repo::LoadOutcome::RecoveredCorrupt(moved) = &outcome {
        log::warn!(
            "settings were corrupt; moved to {} and reset to defaults",
            moved.display()
        );
    }
    (settings, path)
}

/// Whether the OS entry exists (an HKCU Run entry on Windows, a LaunchAgent on macOS). It is the truth: the user
/// may have removed it (Task Manager, Login Items).
fn start_at_login_enabled(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// At start: the OS entry is the truth (the user may have removed it in Task Manager), so the setting follows it.
pub fn reconcile_start_at_login(app: &AppHandle, cat: &CatService) {
    let on = start_at_login_enabled(app);
    if on != cat.settings().start_at_login {
        cat.update_settings(|s| s.start_at_login = on);
    }
}

/// Adds or removes the OS entry, then records the choice in settings. On failure settings are left alone.
pub fn set_start_at_login(app: &AppHandle, cat: &CatService, on: bool) -> Result<(), String> {
    let launcher = app.autolaunch();
    let result = if on {
        launcher.enable()
    } else {
        launcher.disable()
    };
    match result {
        Ok(()) => {
            log::info!("start at login {}", if on { "enabled" } else { "disabled" });
            cat.update_settings(|s| s.start_at_login = on);
            Ok(())
        }
        Err(e) => {
            log::warn!("could not change start at login: {e}");
            Err(e.to_string())
        }
    }
}
