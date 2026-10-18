//! Other OSes: no Space hook, so the cat cannot be dragged.

use super::{Msg, Permission};
use klat_core::geometry::{Physical, Point};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

/// Only Windows needs short names for the Codex hooks; `sh` takes the quoted path.
pub fn cmd_safe_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

pub fn cursor() -> Option<Point<Physical, i32>> {
    None
}
pub fn primary_button_down() -> bool {
    false
}
pub fn replay_space() {}
pub fn permission() -> Permission {
    Permission::NotNeeded
}
pub fn request_permission() {}
pub fn install_hook(
    _app: tauri::AppHandle,
    _tx: Sender<Msg>,
    _on_permission: impl Fn(Permission) + Send + 'static,
) {
}
