//! Names of the events the app emits to the webviews, and the subscriber that sends the cat's state and the
//! settings to them.

use crate::services;
use klat_core::cat::CatState;
use klat_core::settings::Settings;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch::Receiver;

/// The cat's state (payload: `CatState`).
pub const EVENT_STATE: &str = "cat://state";
/// The settings in effect (payload: `Settings`).
pub const EVENT_SETTINGS: &str = "cat://settings";
/// Where the cursor is, for head tracking (see tasks::cursor_poller).
pub const EVENT_CURSOR: &str = "cat://cursor";
/// Dragging the cat with Space: "idle" | "armed" | "dragging" (see tasks::drag_loop).
pub const EVENT_GRAB: &str = "cat://grab";
/// macOS: the Accessibility permission the Space hook needs changed (payload: `platform::Permission`).
pub const EVENT_PERMISSION: &str = "cat://drag-permission";
/// Size slider being dragged: the scale to show, as a factor of the page's size at `PREVIEW_ZOOM`; null when
/// the drag ends.
pub const EVENT_SCALE_PREVIEW: &str = "cat://scale-preview";

/// Sends every published snapshot as `cat://state` and every settings change as `cat://settings`.
pub fn forward(app: &AppHandle, state: Receiver<CatState>, settings: Receiver<Settings>) {
    let handle = app.clone();
    services::on_each(state, move |s| {
        let _ = handle.emit(EVENT_STATE, s);
    });
    let handle = app.clone();
    services::on_each(settings, move |s| {
        let _ = handle.emit(EVENT_SETTINGS, s);
    });
}
