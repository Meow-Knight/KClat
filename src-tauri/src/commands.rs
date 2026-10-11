//! Commands the overlay frontend invokes.

use crate::cursor::Rect;
use crate::settings::Settings;
use crate::state::{self, AppState, CatState};
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

/// Bubble clicked: same as tray "Dismiss all".
#[tauri::command]
pub fn dismiss(app: AppHandle) {
    state::dismiss_all(&app);
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
