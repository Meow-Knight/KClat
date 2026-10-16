//! Everything that differs by OS, and every `unsafe` and FFI call (docs/ARCHITECTURE.md §5.4). The API is here;
//! each OS has its own file: windows.rs, macos.rs, and fallback.rs for the others.
//!
//! The Space hook (dragging the cat, see tasks::drag_loop): Space pressed over the cat with no modifier starts a
//! grab and is kept from the focused app; its release ends it; a plain tap is replayed.

use klat_core::geometry::{Logical, Physical, Point, Rect};
use serde::Serialize;
use std::sync::atomic::AtomicBool;
use std::sync::RwLock;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Runtime, WebviewWindowBuilder, Wry};

pub mod webview_memory;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

#[cfg(not(any(windows, target_os = "macos")))]
mod fallback;
#[cfg(not(any(windows, target_os = "macos")))]
use fallback as os;

pub use os::{
    cursor, install_hook, permission, primary_button_down, replay_space, request_permission,
};
pub use webview_memory::scrub_env;

/// Whether the Space hook may run. Only macOS asks (Accessibility); elsewhere it is not needed.
// Each platform builds only some of these: macOS never says NotNeeded, the others say nothing else.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(test, derive(ts_rs::TS), ts(rename = "DragPermission"))]
pub enum Permission {
    NotNeeded,
    Granted,
    Missing,
}

// ZONE and GRABBING are statics (docs/ARCHITECTURE.md §7.2): the keyboard hook reads them inside the OS callback
// (WH_KEYBOARD_LL on Windows, the CGEventTap on macOS), which gets no state of ours.

/// Where Space starts a grab: the overlay's clickable regions in physical screen px (kept by the cursor poller).
static ZONE: RwLock<Vec<Rect<Physical, i32>>> = RwLock::new(Vec::new());
/// Space is held and kept from the focused app.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
static GRABBING: AtomicBool = AtomicBool::new(false);

/// Called by the cursor poller with the overlay's top-left, its scale factor and its clickable regions.
pub fn set_zone(origin: Point<Physical>, scale: f64, rects: &[Rect<Logical>]) {
    let zone: Vec<_> = rects.iter().map(|r| r.to_physical(origin, scale)).collect();
    if let Ok(mut z) = ZONE.write() {
        if *z != zone {
            *z = zone;
        }
    }
}

/// Sent by the platform hook.
#[cfg_attr(not(any(windows, target_os = "macos")), allow(dead_code))]
pub enum Msg {
    GrabStarted,
    GrabEnded,
}

/// macOS: a menu bar app, without a Dock icon or an app menu.
pub fn menu_bar_app(app: &mut tauri::App) {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// macOS: stay on every desktop (Space), as the overlay does on Windows virtual desktops.
pub fn on_every_desktop<'a, R: Runtime, M: Manager<R>>(
    builder: WebviewWindowBuilder<'a, R, M>,
) -> WebviewWindowBuilder<'a, R, M> {
    #[cfg(target_os = "macos")]
    let builder = builder.visible_on_all_workspaces(true);
    builder
}

/// Windows: the app icon. macOS: a black template image that the menu bar tints for light and dark mode
/// (drawn by scripts/make-tray-template.py).
#[cfg(target_os = "macos")]
pub fn tray_icon(_app: &AppHandle, builder: TrayIconBuilder<Wry>) -> TrayIconBuilder<Wry> {
    builder
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true)
}

#[cfg(not(target_os = "macos"))]
pub fn tray_icon(app: &AppHandle, builder: TrayIconBuilder<Wry>) -> TrayIconBuilder<Wry> {
    match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_uses_the_names_the_settings_window_expects() {
        let names: Vec<serde_json::Value> = [
            Permission::NotNeeded,
            Permission::Granted,
            Permission::Missing,
        ]
        .iter()
        .map(|p| serde_json::to_value(p).unwrap())
        .collect();
        assert_eq!(names, ["not-needed", "granted", "missing"]);
    }

    #[test]
    fn this_platform_reports_its_permission() {
        let expected = if cfg!(target_os = "macos") {
            [Permission::Granted, Permission::Missing].contains(&permission())
        } else {
            permission() == Permission::NotNeeded
        };
        assert!(expected);
    }
}
