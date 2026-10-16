//! Settings: reading and editing them, start at login, the Settings window.

use crate::services::cat_service::CatService;
use crate::services::settings_service;
use crate::ui::overlay::{self, OverlayState};
use klat_core::settings::{Settings, SettingsPatch};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_settings(cat: State<'_, Arc<CatService>>) -> Settings {
    cat.settings()
}

/// Settings window edits: only the fields of `SettingsPatch`, read and clamped like a
/// loaded file. Returns the settings now in effect.
#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    cat: State<'_, Arc<CatService>>,
    overlay_state: State<'_, Arc<OverlayState>>,
    patch: SettingsPatch,
) -> Settings {
    let scale_before = cat.settings().cat_scale;
    let next = cat.update_settings(|s| *s = s.apply(&patch));
    // After a slider drag the overlay resizes itself (with the cat hidden) through `overlay_preview_zoom`.
    if overlay::end_preview(&app, &overlay_state) || next.cat_scale == scale_before {
        return next;
    }
    match overlay::rescale(&app, &cat, &overlay_state, next.zoom()) {
        // A dragged cat keeps its spot: save the window's new top-left.
        Some(pos) => cat.update_settings(|s| s.position = Some(pos)),
        None => next,
    }
}

/// Returns the state that is now in effect; on failure the setting is unchanged and the error is the OS reason.
#[tauri::command]
pub fn set_start_at_login(
    app: AppHandle,
    cat: State<'_, Arc<CatService>>,
    on: bool,
) -> Result<bool, String> {
    settings_service::set_start_at_login(&app, &cat, on).map(|()| on)
}

/// Opens (or brings forward) the Settings window. Async: creating a window from a sync command can deadlock
/// on Windows.
#[tauri::command]
pub async fn open_settings(app: AppHandle, cat: State<'_, Arc<CatService>>) -> Result<(), String> {
    crate::ui::settings_window::open(&app, cat.settings().ui_language).map_err(|e| e.to_string())
}
