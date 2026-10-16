//! Tray menu: quick actions only (docs/specs/settings-i18n-macos.md §4.4). Everything else lives in the Settings
//! window. Labels and tooltip follow the Platform Language and change as soon as it does.

use crate::platform;
use crate::services::cat_service::CatService;
use klat_core::cat::Command;
use klat_core::i18n::{t, tf};
use klat_core::settings::{Settings, UiLanguage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
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

pub fn create(app: &AppHandle, cat: Arc<CatService>) -> tauri::Result<()> {
    let s = cat.settings();
    let lang = s.ui_language;
    let check = |id: &str, key: &str, on: bool| {
        CheckMenuItem::with_id(app, id, t(lang, key), true, on, None::<&str>)
    };
    let plain = |id: &str, key: &str| MenuItem::with_id(app, id, t(lang, key), true, None::<&str>);
    let items = Items {
        ready: check(READY, "tray.ready", cat.snapshot().ready)?,
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
        .on_menu_event({
            let cat = cat.clone();
            move |app, event| on_menu(app, &cat, &event)
        })
        // Windows: double-clicking the tray icon opens Settings.
        .on_tray_icon_event(move |tray, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                open_settings(tray.app_handle(), &cat);
            }
        });
    builder = platform::tray_icon(app, builder);
    builder.build(app)?;
    Ok(())
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
    // The items are made before the tray icon: without them there is no tooltip to set yet.
    let Some(items) = ITEMS.get() else {
        return;
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tooltip(items.lang(), count)));
    }
}

fn on_menu(app: &AppHandle, cat: &CatService, event: &MenuEvent) {
    let Some(items) = ITEMS.get() else {
        return;
    };
    // Check items flip themselves before this runs; read the new state from the item.
    let checked = |item: &CheckMenuItem<Wry>| item.is_checked().unwrap_or(false);
    match event.id().as_ref() {
        READY => {
            cat.dispatch(Command::SetReady(checked(&items.ready)));
        }
        PAUSE => {
            let paused = checked(&items.pause);
            cat.update_settings(|s| s.notifications_enabled = !paused);
        }
        DISMISS_ALL => {
            cat.dispatch(Command::DismissAll);
        }
        SETTINGS => open_settings(app, cat),
        QUIT => app.exit(0),
        _ => {}
    }
}

fn open_settings(app: &AppHandle, cat: &CatService) {
    if let Err(e) = crate::ui::settings_window::open(app, cat.settings().ui_language) {
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
