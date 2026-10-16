//! Tray menu: quick actions only (docs/specs/settings-i18n-macos.md §4.4). Everything else lives in the Settings
//! window. Labels and tooltip follow the Platform Language and change as soon as it does.

use crate::platform;
use crate::services::{self, cat_service::CatService};
use klat_core::cat::{CatState, Command};
use klat_core::i18n::{t, tf};
use klat_core::settings::{Settings, UiLanguage};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tokio::sync::watch::Receiver;

const TRAY_ID: &str = "main";

/// What a menu item does; its id in the menu is `id()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayAction {
    Ready,
    Pause,
    DismissAll,
    Settings,
    Quit,
}

impl TrayAction {
    const ALL: [Self; 5] = [
        Self::Ready,
        Self::Pause,
        Self::DismissAll,
        Self::Settings,
        Self::Quit,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Pause => "pause",
            Self::DismissAll => "dismiss-all",
            Self::Settings => "settings",
            Self::Quit => "quit",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }

    /// The catalog key of its label.
    fn label(self) -> &'static str {
        match self {
            Self::Ready => "tray.ready",
            Self::Pause => "tray.pause",
            Self::DismissAll => "tray.dismissAll",
            Self::Settings => "tray.settings",
            Self::Quit => "tray.quit",
        }
    }
}

/// The menu items and what the tooltip shows, so changes made elsewhere (the overlay button, the Settings window)
/// can update them. Made by `create`, then shared by the menu handler and the subscribers in `follow`.
pub struct TrayHandles {
    ready: CheckMenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    dismiss: MenuItem<Wry>,
    settings: MenuItem<Wry>,
    quit: MenuItem<Wry>,
    lang: Mutex<UiLanguage>,
    /// Sessions waiting, so the tooltip can be rewritten when the language changes.
    waiting: AtomicUsize,
}

impl TrayHandles {
    fn lang(&self) -> UiLanguage {
        *self.lang.lock().unwrap()
    }

    fn relabel(&self, lang: UiLanguage) {
        let _ = self.ready.set_text(t(lang, TrayAction::Ready.label()));
        let _ = self.pause.set_text(t(lang, TrayAction::Pause.label()));
        let _ = self
            .dismiss
            .set_text(t(lang, TrayAction::DismissAll.label()));
        let _ = self
            .settings
            .set_text(t(lang, TrayAction::Settings.label()));
        let _ = self.quit.set_text(t(lang, TrayAction::Quit.label()));
    }
}

pub fn create(app: &AppHandle, cat: Arc<CatService>) -> tauri::Result<Arc<TrayHandles>> {
    let s = cat.settings();
    let lang = s.ui_language;
    let check = |action: TrayAction, on: bool| {
        CheckMenuItem::with_id(
            app,
            action.id(),
            t(lang, action.label()),
            true,
            on,
            None::<&str>,
        )
    };
    let plain = |action: TrayAction| {
        MenuItem::with_id(
            app,
            action.id(),
            t(lang, action.label()),
            true,
            None::<&str>,
        )
    };
    let tray = Arc::new(TrayHandles {
        ready: check(TrayAction::Ready, cat.snapshot().ready)?,
        pause: check(TrayAction::Pause, !s.notifications_enabled)?,
        dismiss: plain(TrayAction::DismissAll)?,
        settings: plain(TrayAction::Settings)?,
        quit: plain(TrayAction::Quit)?,
        lang: Mutex::new(lang),
        waiting: AtomicUsize::new(0),
    });
    let menu = Menu::with_items(
        app,
        &[
            &tray.ready,
            &tray.pause,
            &tray.dismiss,
            &PredefinedMenuItem::separator(app)?,
            &tray.settings,
            &PredefinedMenuItem::separator(app)?,
            &tray.quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(tooltip(lang, 0))
        .menu(&menu)
        .on_menu_event({
            let (cat, tray) = (cat.clone(), tray.clone());
            move |app, event| on_menu(app, &cat, &tray, &event)
        })
        // Windows: double-clicking the tray icon opens Settings.
        .on_tray_icon_event(move |icon, event| {
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                open_settings(icon.app_handle(), &cat);
            }
        });
    builder = platform::tray_icon(app, builder);
    builder.build(app)?;
    Ok(tray)
}

fn set_checked(item: &CheckMenuItem<Wry>, on: bool) {
    if item.is_checked().unwrap_or(!on) != on {
        let _ = item.set_checked(on);
    }
}

/// Keeps the tray in step with the cat (sessions waiting in the tooltip, the Ready tick) and the settings (the
/// Pause tick, the language).
pub fn follow(
    app: &AppHandle,
    tray: Arc<TrayHandles>,
    state: Receiver<CatState>,
    settings: Receiver<Settings>,
) {
    let (handle, handles) = (app.clone(), tray.clone());
    services::on_each(state, move |s| {
        set_waiting(&handle, &handles, s.sessions.len());
        set_checked(&handles.ready, s.ready);
    });
    let handle = app.clone();
    services::on_each(settings, move |s| sync_settings(&handle, &tray, &s));
}

/// Keeps the Pause tick and the labels in step with settings changed elsewhere (the Settings window).
fn sync_settings(app: &AppHandle, tray: &TrayHandles, s: &Settings) {
    set_checked(&tray.pause, !s.notifications_enabled);
    let previous = std::mem::replace(&mut *tray.lang.lock().unwrap(), s.ui_language);
    if previous != s.ui_language {
        tray.relabel(s.ui_language);
        set_waiting(app, tray, tray.waiting.load(Ordering::Relaxed));
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
fn set_waiting(app: &AppHandle, tray: &TrayHandles, count: usize) {
    tray.waiting.store(count, Ordering::Relaxed);
    if let Some(icon) = app.tray_by_id(TRAY_ID) {
        let _ = icon.set_tooltip(Some(tooltip(tray.lang(), count)));
    }
}

fn on_menu(app: &AppHandle, cat: &CatService, tray: &TrayHandles, event: &MenuEvent) {
    // Check items flip themselves before this runs; read the new state from the item.
    let checked = |item: &CheckMenuItem<Wry>| item.is_checked().unwrap_or(false);
    let Some(action) = TrayAction::from_id(event.id().as_ref()) else {
        return;
    };
    match action {
        TrayAction::Ready => {
            cat.dispatch(Command::SetReady(checked(&tray.ready)));
        }
        TrayAction::Pause => {
            let paused = checked(&tray.pause);
            cat.update_settings(|s| s.notifications_enabled = !paused);
        }
        TrayAction::DismissAll => {
            cat.dispatch(Command::DismissAll);
        }
        TrayAction::Settings => open_settings(app, cat),
        TrayAction::Quit => app.exit(0),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_are_the_ones_the_menu_had_and_read_back() {
        let ids: Vec<_> = TrayAction::ALL.iter().map(|a| a.id()).collect();
        assert_eq!(ids, ["ready", "pause", "dismiss-all", "settings", "quit"]);
        for action in TrayAction::ALL {
            assert_eq!(TrayAction::from_id(action.id()), Some(action));
        }
        assert_eq!(TrayAction::from_id("about"), None);
    }

    #[test]
    fn every_label_has_a_text() {
        for lang in [UiLanguage::En, UiLanguage::Vi] {
            for action in TrayAction::ALL {
                assert_ne!(
                    t(lang, action.label()),
                    action.label(),
                    "{action:?} in {lang:?}"
                );
            }
        }
    }
}
