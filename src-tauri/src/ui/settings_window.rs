//! The Settings window (docs/specs/settings-i18n-macos.md §4.1): one at a time, created when asked for and
//! destroyed when closed, so its webview costs no memory while it is not open.

use crate::platform;
use crate::services;
use klat_core::i18n::t;
use klat_core::settings::{Settings, UiLanguage};
use tauri::{
    AppHandle, LogicalUnit, Manager, PixelUnit, WebviewUrl, WebviewWindowBuilder,
    WindowSizeConstraints,
};

pub const LABEL: &str = "settings";
const WIDTH: f64 = 780.0;
const HEIGHT: f64 = 540.0;
const MIN_WIDTH: f64 = 680.0;
const MIN_HEIGHT: f64 = 480.0;
/// The page is one column of settings; wider than this only adds empty space. Height stays free.
const MAX_WIDTH: f64 = 960.0;

/// Shows the Settings window, creating it if needed, and brings it to the front.
pub fn open(app: &AppHandle, lang: UiLanguage) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.unminimize();
        win.show()?;
        return win.set_focus();
    }
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title(t(lang, "settings.windowTitle"))
        .inner_size(WIDTH, HEIGHT)
        .inner_size_constraints(WindowSizeConstraints {
            min_width: Some(logical(MIN_WIDTH)),
            min_height: Some(logical(MIN_HEIGHT)),
            max_width: Some(logical(MAX_WIDTH)),
            max_height: None,
        })
        .resizable(true)
        .maximizable(false)
        .center()
        .additional_browser_args(platform::webview_memory::BROWSER_ARGS)
        .focused(true)
        .visible(true)
        .build()?;
    win.set_focus()
}

fn logical(px: f64) -> PixelUnit {
    PixelUnit::Logical(LogicalUnit::new(px))
}

/// The window title follows the Platform Language (the page sets its own text).
pub fn follow(app: &AppHandle, settings: tokio::sync::watch::Receiver<Settings>) {
    let handle = app.clone();
    services::on_each(settings, move |s| sync_title(&handle, s.ui_language));
}

fn sync_title(app: &AppHandle, lang: UiLanguage) {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.set_title(&t(lang, "settings.windowTitle"));
    }
}
