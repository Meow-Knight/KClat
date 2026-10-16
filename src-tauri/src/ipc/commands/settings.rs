//! Settings: reading and editing them, start at login, the Settings window.

use crate::services::autostart;
use crate::services::cat_service::{self, AppState};
use crate::ui::overlay;
use klat_core::settings::{Settings, SettingsPatch};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

/// Settings window edits: only the fields of `SettingsPatch`, read and clamped like a
/// loaded file. Returns the settings now in effect.
#[tauri::command]
pub fn set_settings(app: AppHandle, patch: SettingsPatch) -> Settings {
    let scale_before = app.state::<AppState>().settings().cat_scale;
    let next = cat_service::update_settings(&app, |s| *s = s.apply(&patch));
    // After a slider drag the overlay resizes itself (with the cat hidden) through `overlay_preview_zoom`.
    if overlay::end_preview(&app) || next.cat_scale == scale_before {
        return next;
    }
    match overlay::rescale(&app, next.zoom()) {
        // A dragged cat keeps its spot: save the window's new top-left.
        Some(pos) => cat_service::update_settings(&app, |s| s.position = Some(pos)),
        None => next,
    }
}

/// Returns the state that is now in effect; on failure the setting is unchanged and the error is the OS reason.
#[tauri::command]
pub fn set_start_at_login(app: AppHandle, on: bool) -> Result<bool, String> {
    autostart::set(&app, on).map(|()| on)
}

/// Opens (or brings forward) the Settings window. Async: creating a window from a sync command can deadlock
/// on Windows.
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    crate::ui::settings_window::open(&app).map_err(|e| e.to_string())
}
