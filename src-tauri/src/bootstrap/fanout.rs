//! Transitional, until ClauCat-dv6.5.3 hands each of these to its owner: what state.rs pushed out after every
//! change (publish_state, update_settings), now driven by the service's watch channels.

use crate::ipc::events::{EVENT_SETTINGS, EVENT_STATE};
use crate::platform::webview_memory;
use crate::services::cat_service::CatService;
use crate::ui::{settings_window, tray};
use tauri::{AppHandle, Emitter};

/// Subscribes now: call it before anything can change the state.
pub fn start(app: &AppHandle, cat: &CatService) {
    let mut state = cat.subscribe();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut awake = state.borrow().awake;
        while state.changed().await.is_ok() {
            // Cloned out at once: holding the borrow would block the next dispatch.
            let snapshot = state.borrow_and_update().clone();
            if snapshot.awake != awake {
                awake = snapshot.awake;
                webview_memory::awake_changed(&handle, awake);
            }
            tray::set_waiting(&handle, snapshot.sessions.len());
            tray::set_ready_checked(snapshot.ready);
            let _ = handle.emit(EVENT_STATE, snapshot);
        }
    });

    let mut settings = cat.subscribe_settings();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while settings.changed().await.is_ok() {
            let next = settings.borrow_and_update().clone();
            tray::sync_settings(&handle, &next);
            settings_window::sync_title(&handle, next.ui_language);
            let _ = handle.emit(EVENT_SETTINGS, next);
        }
    });
}
