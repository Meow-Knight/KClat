//! Tray menu (spec §2.3).

use crate::autostart;
use crate::hooks_installer::{Action, Status};
use crate::hooks_ui;
use crate::settings::Settings;
use crate::state::{self, AppState};
use std::sync::OnceLock;
use std::thread;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
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
const QUIT: &str = "quit";
const TRAY_ID: &str = "main";

/// The menu items, so changes made elsewhere (the overlay button, the Settings window) can update their ticks.
static ITEMS: OnceLock<Items> = OnceLock::new();

struct Items {
    ready: CheckMenuItem<Wry>,
    sfw: CheckMenuItem<Wry>,
    pause: CheckMenuItem<Wry>,
    hide: CheckMenuItem<Wry>,
    autostart: CheckMenuItem<Wry>,
    hooks: Submenu<Wry>,
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let s = app.state::<AppState>().settings();
    // The OS entry is the truth (the user may have removed it in Task Manager); keep the setting in step.
    let autostart_on = autostart::is_enabled(app);
    if autostart_on != s.start_at_login {
        state::update_settings(app, |s| s.start_at_login = autostart_on);
    }

    let items = Items {
        ready: CheckMenuItem::with_id(
            app,
            READY,
            "Ready: wake me when Claude is done",
            true,
            false,
            None::<&str>,
        )?,
        sfw: CheckMenuItem::with_id(app, SFW, "SFW mode", true, s.sfw_mode, None::<&str>)?,
        pause: CheckMenuItem::with_id(
            app,
            PAUSE,
            "Pause notifications",
            true,
            !s.notifications_enabled,
            None::<&str>,
        )?,
        hide: CheckMenuItem::with_id(
            app,
            HIDE,
            "Hide cat while sleeping",
            true,
            s.hide_while_sleeping,
            None::<&str>,
        )?,
        autostart: CheckMenuItem::with_id(
            app,
            AUTOSTART,
            "Start at login",
            true,
            autostart_on,
            None::<&str>,
        )?,
        hooks: Submenu::with_items(
            app,
            hooks_title(hooks_ui::current_status()),
            true,
            &[
                &MenuItem::with_id(app, HOOKS_INSTALL, "Install", true, None::<&str>)?,
                &MenuItem::with_id(app, HOOKS_REPAIR, "Repair", true, None::<&str>)?,
                &MenuItem::with_id(app, HOOKS_UNINSTALL, "Uninstall", true, None::<&str>)?,
            ],
        )?,
    };
    let menu = Menu::with_items(
        app,
        &[
            &items.ready,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, WAKE_TEST, "Wake test", true, None::<&str>)?,
            &MenuItem::with_id(app, SLEEP, "Sleep", true, None::<&str>)?,
            &MenuItem::with_id(app, DISMISS_ALL, "Dismiss all", true, None::<&str>)?,
            &MenuItem::with_id(
                app,
                RESET_POSITION,
                "Move the cat back to the corner",
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &items.sfw,
            &items.pause,
            &items.hide,
            &items.autostart,
            &PredefinedMenuItem::separator(app)?,
            &items.hooks,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT, "Quit", true, None::<&str>)?,
        ],
    )?;

    let _ = ITEMS.set(items);
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Klat")
        .menu(&menu)
        .on_menu_event(|app, event| on_menu(app, &event));
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

/// Keeps the ticks in step with settings changed elsewhere (the Settings window).
pub fn sync_settings(s: &Settings) {
    if let Some(items) = ITEMS.get() {
        set_checked(&items.sfw, s.sfw_mode);
        set_checked(&items.pause, !s.notifications_enabled);
        set_checked(&items.hide, s.hide_while_sleeping);
        set_checked(&items.autostart, s.start_at_login);
    }
}

/// After the hooks were installed or removed from somewhere else.
pub fn refresh_hooks_title() {
    if let Some(items) = ITEMS.get() {
        let _ = items
            .hooks
            .set_text(hooks_title(hooks_ui::current_status()));
    }
}

/// Tooltip shows how many sessions are waiting, e.g. "Klat · 2 sessions waiting".
pub fn set_waiting(app: &AppHandle, count: usize) {
    let tip = match count {
        0 => "Klat".to_owned(),
        1 => "Klat · 1 session waiting".to_owned(),
        n => format!("Klat · {n} sessions waiting"),
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(tip));
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
        QUIT => app.exit(0),
        _ => {}
    }
}

fn set_autostart(app: &AppHandle, on: bool, item: &CheckMenuItem<Wry>) {
    if let Err(e) = autostart::set(app, on) {
        let _ = item.set_checked(!on);
        notify_error(app, format!("Could not change \"Start at login\": {e}"));
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

fn notify_error(app: &AppHandle, text: impl Into<String>) {
    app.dialog()
        .message(text)
        .title("Klat")
        .kind(MessageDialogKind::Error)
        .show(|_| {});
}

fn hooks_title(status: Status) -> &'static str {
    match status {
        Status::Installed => "Hooks (installed)",
        Status::NeedsRepair => "Hooks (needs repair)",
        Status::NotInstalled => "Hooks (not installed)",
    }
}

/// File I/O off the menu thread; the outcome is reported in a message box.
fn run_installer(app: &AppHandle, action: Action) {
    let app = app.clone();
    thread::spawn(move || {
        let (kind, text) = match hooks_ui::apply(action) {
            Ok(text) => (MessageDialogKind::Info, text),
            Err(text) => (MessageDialogKind::Error, text),
        };
        refresh_hooks_title();
        app.dialog()
            .message(text)
            .title("Klat")
            .kind(kind)
            .show(|_| {});
    });
}
