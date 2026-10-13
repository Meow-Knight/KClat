// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod commands;
mod cursor;
// Shared with the hook adapter so both always agree on where runtime.json lives.
#[path = "../../crates/klat-hook/src/data_dir.rs"]
mod data_dir;
mod drag;
mod event_server;
mod hooks_installer;
mod hooks_ui;
mod i18n;
mod log;
mod overlay;
mod session_store;
mod settings;
mod settings_window;
mod state;
mod tray;

use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, RunEvent};

/// How often waiting sessions are checked against `staleAfterMs`.
const STALE_CHECK: Duration = Duration::from_secs(30);

fn main() {
    settings::migrate_legacy_dir();
    log::init(&state::data_dir());

    let app = tauri::Builder::default()
        // Must come first: a second launch hands over to this instance and exits before creating anything.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info("second launch ignored: already running");
            tray::notify(app, i18n::t(i18n::current(app), "dialog.alreadyRunning"));
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::load())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_settings,
            commands::dismiss,
            commands::set_hitboxes,
            commands::set_ready,
            commands::set_settings,
            commands::set_start_at_login,
            commands::wake_test,
            commands::sleep,
            commands::hooks_status,
            commands::hooks_apply,
            commands::open_settings,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(win) = overlay::get(window.app_handle()) {
                    overlay::on_scale_changed(&win);
                }
            }
        })
        .setup(|app| {
            // Logged here, not before the builder: a second launch exits inside the single-instance plugin.
            log::info(format!("Klat {} starting", env!("CARGO_PKG_VERSION")));
            // macOS: a menu bar app, without a Dock icon or an app menu.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            overlay::create(app.handle())?;
            autostart::reconcile(app.handle());
            tray::create(app.handle())?;
            cursor::start(app.handle().clone());
            drag::start(app.handle().clone());
            start_event_server(app.handle())?;
            start_stale_ticker(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Klat");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            event_server::remove_runtime(&event_server::runtime_path());
            log::info("exiting");
        }
    });
}

fn start_event_server(app: &AppHandle) -> std::io::Result<()> {
    let token = event_server::new_token()?;
    let handle = app.clone();
    let port = event_server::start(token.clone(), move |ev| {
        state::apply_hook_event(&handle, &ev)
    })?;
    log::info(format!("event server listening on 127.0.0.1:{port}"));
    event_server::write_runtime(
        &event_server::runtime_path(),
        &event_server::RuntimeInfo {
            port,
            token,
            pid: std::process::id(),
        },
    )
}

fn start_stale_ticker(app: AppHandle) {
    thread::Builder::new()
        .name("stale-ticker".into())
        .spawn(move || loop {
            thread::sleep(STALE_CHECK);
            state::expire_stale(&app);
        })
        .expect("failed to start the stale ticker");
}
