//! Commands the overlay and the Settings window invoke.

use crate::autostart;
use crate::hooks_installer::{Action, Status};
use crate::hooks_ui;
use crate::i18n;
use crate::overlay;
use crate::settings::{self, Settings};
use crate::state::{self, AppState, CatState};
use klat_core::geometry::Rect;
use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager, State};

/// Upper bound on reported hitboxes; the overlay sends at most cat + bubble.
const MAX_HITBOXES: usize = 8;

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> CatState {
    state.snapshot()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

/// Settings window edits. Only the fields in `settings::PATCHABLE` are taken, and they are validated like a
/// loaded file. Returns the settings now in effect.
#[tauri::command]
pub fn set_settings(app: AppHandle, patch: Map<String, Value>) -> Settings {
    let scale_before = app.state::<AppState>().settings().cat_scale;
    let next = state::update_settings(&app, |s| *s = settings::apply_patch(s, &patch));
    // After a slider drag the overlay resizes itself (with the cat hidden) through `overlay_preview_zoom`.
    if overlay::end_preview(&app) || next.cat_scale == scale_before {
        return next;
    }
    match overlay::rescale(&app, next.zoom()) {
        // A dragged cat keeps its spot: save the window's new top-left.
        Some(pos) => state::update_settings(&app, |s| s.position = Some(pos)),
        None => next,
    }
}

/// The size slider is being dragged: show `scale` (percent) in the overlay; nothing is saved until
/// `set_settings`.
#[tauri::command]
pub fn preview_cat_scale(app: AppHandle, scale: u32) {
    overlay::preview(&app, scale);
}

/// The overlay page, with the cat hidden, switches its window to the preview size (`preview`) or back to the
/// cat scale. See `overlay::preview_zoom`.
#[tauri::command]
pub fn overlay_preview_zoom(app: AppHandle, preview: bool) -> overlay::ZoomChange {
    let (change, saved) = overlay::preview_zoom(&app, preview);
    if let Some(pos) = saved {
        state::update_settings(&app, |s| s.position = Some(pos));
    }
    change
}

/// Returns the state that is now in effect; on failure the setting is unchanged and the error is the OS reason.
#[tauri::command]
pub fn set_start_at_login(app: AppHandle, on: bool) -> Result<bool, String> {
    autostart::set(&app, on).map(|()| on)
}

#[tauri::command]
pub fn wake_test(app: AppHandle) {
    state::wake_test(&app);
}

/// Same as the tray's Sleep: dismisses whatever is waiting. Sends nothing to Claude Code.
#[tauri::command]
pub fn sleep(app: AppHandle) {
    state::dismiss_all(&app);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HooksInfo {
    pub status: Status,
    /// `~/.claude/settings.json`; None when the home folder is unknown.
    pub settings_path: Option<String>,
}

#[tauri::command]
pub fn hooks_status() -> HooksInfo {
    HooksInfo {
        status: hooks_ui::current_status(),
        settings_path: hooks_ui::paths().map(|(s, _)| s.display().to_string()),
    }
}

#[derive(Debug, Serialize)]
pub struct HooksResult {
    pub ok: bool,
    pub message: String,
}

/// `action`: "install" (also repairs) or "uninstall". File I/O runs off the main thread; the message is in
/// the Platform Language.
#[tauri::command]
pub async fn hooks_apply(app: AppHandle, action: Action) -> HooksResult {
    let lang = i18n::current(&app);
    let result = tauri::async_runtime::spawn_blocking(move || hooks_ui::apply(lang, action))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match result {
        Ok(message) => HooksResult { ok: true, message },
        Err(message) => HooksResult { ok: false, message },
    }
}

/// Whether dragging the cat needs a permission first (macOS: Accessibility).
#[tauri::command]
pub fn drag_permission() -> crate::drag::Permission {
    crate::drag::permission()
}

/// macOS: asks for the Accessibility permission and opens the pane where it is granted.
#[tauri::command]
pub fn open_drag_permission() {
    crate::drag::request_permission();
}

/// Opens (or brings forward) the Settings window. Async: creating a window from a sync command can deadlock
/// on Windows.
#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Result<(), String> {
    crate::settings_window::open(&app).map_err(|e| e.to_string())
}

/// Bubble clicked: same as tray "Dismiss all".
#[tauri::command]
pub fn dismiss(app: AppHandle) {
    state::dismiss_all(&app);
}

/// The Ready button next to the cat.
#[tauri::command]
pub fn set_ready(app: AppHandle, on: bool) {
    state::set_ready(&app, on);
}

#[tauri::command]
pub fn set_hitboxes(state: State<'_, AppState>, rects: Vec<Rect>) {
    let rects = rects
        .into_iter()
        .filter(|r| [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) && r.w > 0.0 && r.h > 0.0)
        .take(MAX_HITBOXES)
        .collect();
    *state.page_hitboxes.lock().unwrap() = rects;
}
