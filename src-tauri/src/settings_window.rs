//! The Settings window (docs/specs/settings-i18n-macos.md §4.1): one at a time, created when asked for and
//! destroyed when closed, so its webview costs no memory while it is not open.

use crate::i18n;
use crate::settings::UiLanguage;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "settings";
const WIDTH: f64 = 780.0;
const HEIGHT: f64 = 540.0;
const MIN_WIDTH: f64 = 680.0;
const MIN_HEIGHT: f64 = 480.0;

/// Shows the Settings window, creating it if needed, and brings it to the front.
pub fn open(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.unminimize();
        win.show()?;
        return win.set_focus();
    }
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title(i18n::t(i18n::current(app), "settings.windowTitle"))
        .inner_size(WIDTH, HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .resizable(true)
        .center()
        .focused(true)
        .visible(true)
        .build()?;
    win.set_focus()
}

/// The window title follows the Platform Language (the page sets its own text).
pub fn sync_title(app: &AppHandle, lang: UiLanguage) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.set_title(&i18n::t(lang, "settings.windowTitle"));
    }
}
