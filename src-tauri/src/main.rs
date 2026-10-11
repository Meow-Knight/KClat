// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod cursor;
mod event_server;
mod hooks_installer;
mod overlay;
mod session_store;
mod settings;
mod state;
mod tray;

use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, RunEvent};

/// How often waiting sessions are checked against `staleAfterMs`.
const STALE_CHECK: Duration = Duration::from_secs(30);

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::load())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::get_settings,
            commands::dismiss,
            commands::set_hitboxes,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::ScaleFactorChanged { .. } = event {
                if let Some(win) = overlay::get(window.app_handle()) {
                    overlay::on_scale_changed(&win);
                }
            }
        })
        .setup(|app| {
            overlay::create(app.handle())?;
            tray::create(app.handle())?;
            cursor::start(app.handle().clone());
            start_event_server(app.handle())?;
            start_stale_ticker(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Claude Cat");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            event_server::remove_runtime(&event_server::runtime_path());
        }
    });
}

fn start_event_server(app: &AppHandle) -> std::io::Result<()> {
    let token = event_server::new_token()?;
    let handle = app.clone();
    let port = event_server::start(token.clone(), move |ev| {
        state::apply_hook_event(&handle, &ev)
    })?;
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
