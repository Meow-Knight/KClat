//! Other OSes: no Space hook, so the cat cannot be dragged.

use super::{Msg, Permission};
use klat_core::geometry::{Physical, Point};
use std::sync::mpsc::Sender;

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
pub fn install_hook(_app: tauri::AppHandle, _tx: Sender<Msg>) {}
