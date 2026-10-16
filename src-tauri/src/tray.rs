//! Tray menu: quick actions only (docs/specs/settings-i18n-macos.md §4.4). Everything else lives in the Settings
//! window. Labels and tooltip follow the Platform Language and change as soon as it does.

use crate::i18n;
use crate::state::{self, AppState};
use klat_core::i18n::{t, tf};
use klat_core::settings::{Settings, UiLanguage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const READY: &str = "ready";
const PAUSE: &str = "pause";
const DISMISS_ALL: &str = "dismiss-all";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";

/// The menu items, so changes made elsewhere (the overlay button, the Settings window) can update them.
static ITEMS: OnceLock<Items> = OnceLock::new();
/// Sessions waiting, so the tooltip can be rewritten when the language changes.
static WAITING: AtomicUsize = AtomicUsize::new(0);

struct Items {
    ready: CheckMenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    dismiss: MenuItem<Wry>,
    settings: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    lang: Mutex<UiLanguage>,
}

impl Items {
    fn lang(&self) -> UiLanguage {
        *self.lang.lock().unwrap()
    }

    fn relabel(&self, lang: UiLanguage) {
        let _ = self.ready.set_text(t(lang, "tray.ready"));
        let _ = self.pause.set_text(t(lang, "tray.pause"));
        let _ = self.dismiss.set_text(t(lang, "tray.dismissAll"));
        let _ = self.settings.set_text(t(lang, "tray.settings"));
        let _ = self.quit.set_text(t(lang, "tray.quit"));
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let s = state.settings();
    let lang = s.ui_language;
    let check = |id: &str, key: &str, on: bool| {
        CheckMenuItem::with_id(app, id, t(lang, key), true, on, None::<&str>)
    };
    let plain = |id: &str, key: &str| MenuItem::with_id(app, id, t(lang, key), true, None::<&str>);
    let items = Items {
        ready: check(READY, "tray.ready", state.snapshot().ready)?,
        pause: check(PAUSE, "tray.pause", !s.notifications_enabled)?,
        dismiss: plain(DISMISS_ALL, "tray.dismissAll")?,
        settings: plain(SETTINGS, "tray.settings")?,
        quit: plain(QUIT, "tray.quit")?,
        lang: Mutex::new(lang),
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.ready,
            &items.pause,
            &items.dismiss,
            &PredefinedMenuItem::separator(app)?,
            &items.settings,
            &PredefinedMenuItem::separator(app)?,
            &items.quit,
        ],
    )?;
    let _ = ITEMS.set(items);

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip(lang, 0))
        .menu(&menu)
        .on_menu_event(|app, event| on_menu(app, &event))
        // Windows: double-clicking the tray icon opens Settings.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                open_settings(tray.app_handle());
            }
        });
    builder = tray_icon(app, builder);
    builder.build(app)?;
    Ok(())
}

/// Windows: the app icon. macOS: a black template image that the menu bar tints for light and dark mode
/// (drawn by scripts/make-tray-template.py).
#[cfg(target_os = "macos")]
fn tray_icon(_app: &AppHandle, builder: TrayIconBuilder<Wry>) -> TrayIconBuilder<Wry> {
    builder
        .icon(tauri::include_image!("icons/tray-template.png"))
        .icon_as_template(true)
}

#[cfg(not(target_os = "macos"))]
fn tray_icon(app: &AppHandle, builder: TrayIconBuilder<Wry>) -> TrayIconBuilder<Wry> {
    match app.default_window_icon() {
        Some(icon) => builder.icon(icon.clone()),
        None => builder,
    }
}

fn set_checked(item: &CheckMenuItem<Wry>, on: bool) {
    if item.is_checked().unwrap_or(!on) != on {
        let _ = item.set_checked(on);
    }
}

/// Keeps the tray tick in step with the Ready state.
pub fn set_ready_checked(on: bool) {
    if let Some(items) = ITEMS.get() {
        set_checked(&items.ready, on);
    }
}

/// Keeps the Pause tick and the labels in step with settings changed elsewhere (the Settings window).
pub fn sync_settings(app: &AppHandle, s: &Settings) {
    let Some(items) = ITEMS.get() else {
        return;
    };
    set_checked(&items.pause, !s.notifications_enabled);
    let previous = std::mem::replace(&mut *items.lang.lock().unwrap(), s.ui_language);
    if previous != s.ui_language {
        items.relabel(s.ui_language);
        set_waiting(app, WAITING.load(Ordering::Relaxed));
    }
}

/// "Klat · 2 sessions waiting".
fn tooltip(lang: UiLanguage, count: usize) -> String {
    match count {
        0 => t(lang, "tray.tooltip.idle"),
        1 => t(lang, "tray.tooltip.one"),
        n => tf(lang, "tray.tooltip.many", &[("count", &n.to_string())]),
    }
}

/// Tooltip shows how many sessions are waiting.
pub fn set_waiting(app: &AppHandle, count: usize) {
    WAITING.store(count, Ordering::Relaxed);
    let lang = ITEMS.get().map_or_else(|| i18n::current(app), Items::lang);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(lang, count)));
    }
}

fn on_menu(app: &AppHandle, event: &MenuEvent) {
    let Some(items) = ITEMS.get() else {
        return;
    };
    // Check items flip themselves before this runs; read the new state from the item.
    let checked = |item: &CheckMenuItem<Wry>| item.is_checked().unwrap_or(false);
    match event.id().as_ref() {
        READY => state::set_ready(app, checked(&items.ready)),
        PAUSE => {
            let paused = checked(&items.pause);
            state::update_settings(app, |s| s.notifications_enabled = !paused);
        }
        DISMISS_ALL => state::dismiss_all(app),
        SETTINGS => open_settings(app),
        QUIT => app.exit(0),
        _ => {}
    }
}

fn open_settings(app: &AppHandle) {
    if let Err(e) = crate::settings_window::open(app) {
        log::warn!("could not open settings: {e}");
    }
}

/// A plain message box from the tray app.
pub fn notify(app: &AppHandle, text: impl Into<String>) {
    app.dialog()
        .message(text)
        .title("Klat")
        .kind(MessageDialogKind::Info)
        .show(|_| {});
}
