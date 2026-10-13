//! Commands the overlay and the Settings window invoke.

use crate::autostart;
use crate::cursor::Rect;
use crate::hooks_installer::{Action, Status};
use crate::hooks_ui;
use crate::i18n;
use crate::settings::{self, Settings};
use crate::state::{self, AppState, CatState};
use serde::Serialize;
use serde_json::{Map, Value};
use tauri::{AppHandle, State};

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
    state::update_settings(&app, |s| *s = settings::apply_patch(s, &patch))
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
    crate::tray::refresh_hooks_title();
    match result {
        Ok(message) => HooksResult { ok: true, message },
        Err(message) => HooksResult { ok: false, message },
    }
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
    *state.hitboxes.lock().unwrap() = rects;
}
