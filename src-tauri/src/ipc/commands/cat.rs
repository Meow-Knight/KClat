//! The cat: its state, waking it for a test, Ready, Dismiss and Sleep.

use crate::services::cat_service::CatService;
use klat_core::cat::{CatState, Command};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_state(cat: State<'_, Arc<CatService>>) -> CatState {
    cat.snapshot()
}

#[tauri::command]
pub fn wake_test(cat: State<'_, Arc<CatService>>) {
    cat.dispatch(Command::WakeTest);
}

/// Same as the tray's Sleep: dismisses whatever is waiting. Sends nothing to Claude Code.
#[tauri::command]
pub fn sleep(cat: State<'_, Arc<CatService>>) {
    cat.dispatch(Command::DismissAll);
}

/// Bubble clicked: same as tray "Dismiss all".
#[tauri::command]
pub fn dismiss(cat: State<'_, Arc<CatService>>) {
    cat.dispatch(Command::DismissAll);
}

/// The Ready button next to the cat.
#[tauri::command]
pub fn set_ready(cat: State<'_, Arc<CatService>>, on: bool) {
    cat.dispatch(Command::SetReady(on));
}
