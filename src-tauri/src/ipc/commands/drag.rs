//! The permission dragging the cat needs (macOS: Accessibility).

use crate::platform::{self, Permission};

/// Whether dragging the cat needs a permission first (macOS: Accessibility).
#[tauri::command]
pub fn drag_permission() -> Permission {
    platform::permission()
}

/// macOS: asks for the Accessibility permission and opens the pane where it is granted.
#[tauri::command]
pub fn open_drag_permission() {
    platform::request_permission();
}
