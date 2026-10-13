//! Tray menu (spec §2.3). Labels and tooltip follow the Platform Language and change as soon as it does.

use crate::autostart;
use crate::hooks_installer::{Action, Status};
use crate::hooks_ui;
use crate::i18n::{self, t, tf};
use crate::settings::{Settings, UiLanguage};
use crate::state::{self, AppState};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

const READY: &str = "ready";
const WAKE_TEST: &str = "wake-test";
const SLEEP: &str = "sleep";
const DISMISS_ALL: &str = "dismiss-all";
const RESET_POSITION: &str = "reset-position";
const SFW: &str = "sfw";
const PAUSE: &str = "pause";
const HIDE: &str = "hide-while-sleeping";
const AUTOSTART: &str = "start-at-login";
const HOOKS_INSTALL: &str = "hooks-install";
const HOOKS_REPAIR: &str = "hooks-repair";
const HOOKS_UNINSTALL: &str = "hooks-uninstall";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";

/// The menu items, so changes made elsewhere (the overlay button, the Settings window) can update them.
static ITEMS: OnceLock<Items> = OnceLock::new();
/// Sessions waiting, so the tooltip can be rewritten when the language changes.
static WAITING: AtomicUsize = AtomicUsize::new(0);

struct Items {
    ready: CheckMenuItem<Wry>,
    sfw: CheckMenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    hide: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    hooks: Submenu<Wry>,
    /// Items without a tick, with their catalog key.
    plain: Vec<(MenuItem<Wry>, &'static str)>,
    lang: Mutex<UiLanguage>,
}

impl Items {
    fn lang(&self) -> UiLanguage {
        *self.lang.lock().unwrap()
    }

    fn relabel(&self, lang: UiLanguage) {
        let checks = [
            (&self.ready, "tray.ready"),
            (&self.sfw, "tray.sfw"),
            (&self.pause, "tray.pause"),
            (&self.hide, "tray.hide"),
            (&self.autostart, "tray.startAtLogin"),
        ];
        for (item, key) in checks {
            let _ = item.set_text(t(lang, key));
        }
        for (item, key) in &self.plain {
            let _ = item.set_text(t(lang, key));
        }
        let _ = self
            .hooks
            .set_text(hooks_title(lang, hooks_ui::current_status()));
    }
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let s = app.state::<AppState>().settings();
    // The OS entry is the truth (the user may have removed it in Task Manager); keep the setting in step.
    let autostart_on = autostart::is_enabled(app);
    if autostart_on != s.start_at_login {
        state::update_settings(app, |s| s.start_at_login = autostart_on);
    }
    let lang = s.ui_language;
    let check = |id: &str, key: &str, on: bool| {
        CheckMenuItem::with_id(app, id, t(lang, key), true, on, None::<&str>)
    };
    let plain = |id: &str, key: &'static str| {
        MenuItem::with_id(app, id, t(lang, key), true, None::<&str>).map(|item| (item, key))
    };

    let ready = check(READY, "tray.ready", false)?;
    let sfw = check(SFW, "tray.sfw", s.sfw_mode)?;
    let pause = check(PAUSE, "tray.pause", !s.notifications_enabled)?;
    let hide = check(HIDE, "tray.hide", s.hide_while_sleeping)?;
    let autostart = check(AUTOSTART, "tray.startAtLogin", autostart_on)?;
    let install = plain(HOOKS_INSTALL, "tray.hooks.install")?;
    let repair = plain(HOOKS_REPAIR, "tray.hooks.repair")?;
    let uninstall = plain(HOOKS_UNINSTALL, "tray.hooks.uninstall")?;
    let hooks = Submenu::with_items(
        app,
        hooks_title(lang, hooks_ui::current_status()),
        true,
        &[&install.0, &repair.0, &uninstall.0],
    )?;
    let wake_test = plain(WAKE_TEST, "tray.wakeTest")?;
    let sleep = plain(SLEEP, "tray.sleep")?;
    let dismiss = plain(DISMISS_ALL, "tray.dismissAll")?;
    let reset = plain(RESET_POSITION, "tray.resetPosition")?;
    let settings_item = plain(SETTINGS, "tray.settings")?;
    let quit = plain(QUIT, "tray.quit")?;
    let menu = Menu::with_items(
        app,
        &[
            &ready,
            &PredefinedMenuItem::separator(app)?,
            &wake_test.0,
            &sleep.0,
            &dismiss.0,
            &reset.0,
            &PredefinedMenuItem::separator(app)?,
            &sfw,
            &pause,
            &hide,
            &autostart,
            &PredefinedMenuItem::separator(app)?,
            &hooks,
            &PredefinedMenuItem::separator(app)?,
            &settings_item.0,
            &PredefinedMenuItem::separator(app)?,
            &quit.0,
        ],
    )?;

    let _ = ITEMS.set(Items {
        ready,
        sfw,
        pause,
        hide,
        autostart,
        hooks,
        plain: vec![
            install,
            repair,
            uninstall,
            wake_test,
            sleep,
            dismiss,
            reset,
            settings_item,
            quit,
        ],
        lang: Mutex::new(lang),
    });
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
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
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

/// Keeps ticks and labels in step with settings changed elsewhere (the Settings window).
pub fn sync_settings(app: &AppHandle, s: &Settings) {
    let Some(items) = ITEMS.get() else {
        return;
    };
    set_checked(&items.sfw, s.sfw_mode);
    set_checked(&items.pause, !s.notifications_enabled);
    set_checked(&items.hide, s.hide_while_sleeping);
    set_checked(&items.autostart, s.start_at_login);
    let previous = std::mem::replace(&mut *items.lang.lock().unwrap(), s.ui_language);
    if previous != s.ui_language {
        items.relabel(s.ui_language);
        set_waiting(app, WAITING.load(Ordering::Relaxed));
    }
}

/// After the hooks were installed or removed from somewhere else.
pub fn refresh_hooks_title() {
    if let Some(items) = ITEMS.get() {
        let _ = items
            .hooks
            .set_text(hooks_title(items.lang(), hooks_ui::current_status()));
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
        WAKE_TEST => state::wake_test(app),
        SLEEP | DISMISS_ALL => state::dismiss_all(app),
        RESET_POSITION => {
            state::update_settings(app, |s| s.position = None);
            if let Some(win) = crate::overlay::get(app) {
                let _ = crate::overlay::place(&win, None);
            }
        }
        SFW => {
            let on = checked(&items.sfw);
            state::update_settings(app, |s| s.sfw_mode = on);
        }
        PAUSE => {
            let paused = checked(&items.pause);
            state::update_settings(app, |s| s.notifications_enabled = !paused);
        }
        HIDE => {
            let on = checked(&items.hide);
            state::update_settings(app, |s| s.hide_while_sleeping = on);
        }
        AUTOSTART => set_autostart(app, checked(&items.autostart), &items.autostart),
        HOOKS_INSTALL | HOOKS_REPAIR => run_installer(app, Action::Install),
        HOOKS_UNINSTALL => run_installer(app, Action::Uninstall),
        SETTINGS => open_settings(app),
        QUIT => app.exit(0),
        _ => {}
    }
}

fn open_settings(app: &AppHandle) {
    if let Err(e) = crate::settings_window::open(app) {
        crate::log::warn(format!("could not open settings: {e}"));
    }
}

fn set_autostart(app: &AppHandle, on: bool, item: &CheckMenuItem<Wry>) {
    if let Err(e) = autostart::set(app, on) {
        let _ = item.set_checked(!on);
        let text = tf(
            i18n::current(app),
            "dialog.startAtLoginFailed",
            &[("error", &e)],
        );
        message(app, MessageDialogKind::Error, text);
    }
}

/// A plain message box from the tray app.
pub fn notify(app: &AppHandle, text: impl Into<String>) {
    message(app, MessageDialogKind::Info, text);
}

fn message(app: &AppHandle, kind: MessageDialogKind, text: impl Into<String>) {
    app.dialog()
        .message(text)
        .title("Klat")
        .kind(kind)
        .show(|_| {});
}

fn hooks_title(lang: UiLanguage, status: Status) -> String {
    t(
        lang,
        match status {
            Status::Installed => "tray.hooks.installed",
            Status::NeedsRepair => "tray.hooks.needsRepair",
            Status::NotInstalled => "tray.hooks.notInstalled",
        },
    )
}

/// File I/O off the menu thread; the outcome is reported in a message box.
fn run_installer(app: &AppHandle, action: Action) {
    let app = app.clone();
    thread::spawn(move || {
        let (kind, text) = match hooks_ui::apply(i18n::current(&app), action) {
            Ok(text) => (MessageDialogKind::Info, text),
            Err(text) => (MessageDialogKind::Error, text),
        };
        refresh_hooks_title();
        message(&app, kind, text);
    });
}
