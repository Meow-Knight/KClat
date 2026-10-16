//! The cat: its state, waking it for a test, Ready, Dismiss and Sleep.

use crate::services::cat_service::{self, AppState, CatState};
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> CatState {
    state.snapshot()
}

#[tauri::command]
pub fn wake_test(app: AppHandle) {
    cat_service::wake_test(&app);
}

/// Same as the tray's Sleep: dismisses whatever is waiting. Sends nothing to Claude Code.
#[tauri::command]
pub fn sleep(app: AppHandle) {
    cat_service::dismiss_all(&app);
}

/// Bubble clicked: same as tray "Dismiss all".
#[tauri::command]
pub fn dismiss(app: AppHandle) {
    cat_service::dismiss_all(&app);
}

/// The Ready button next to the cat.
#[tauri::command]
pub fn set_ready(app: AppHandle, on: bool) {
    cat_service::set_ready(&app, on);
}
