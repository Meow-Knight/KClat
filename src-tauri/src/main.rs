// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod commands;
mod cursor;
mod drag;
mod event_server;
mod hooks_installer;
mod hooks_ui;
mod i18n;
mod log;
mod overlay;
mod settings;
mod settings_window;
mod state;
mod tray;
mod webview_memory;

use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Manager, RunEvent};

/// How often waiting sessions are checked against `staleAfterMs`.
const STALE_CHECK: Duration = Duration::from_secs(30);

fn main() {
    webview_memory::scrub_env();
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
            commands::preview_cat_scale,
            commands::overlay_preview_zoom,
            commands::set_start_at_login,
            commands::wake_test,
            commands::sleep,
            commands::hooks_status,
            commands::hooks_apply,
            commands::open_settings,
            commands::drag_permission,
            commands::open_drag_permission,
        ])
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(win) = overlay::get(window.app_handle()) {
                    overlay::on_scale_changed(&win);
                }
            }
            // Closed in the middle of a size slider drag: put the cat back at its saved size.
            tauri::WindowEvent::Destroyed if window.label() == settings_window::LABEL => {
                overlay::end_preview(window.app_handle());
            }
            _ => {}
        })
        .setup(|app| {
            // Logged here, not before the builder: a second launch exits inside the single-instance plugin.
            log::info(format!("Klat {} starting", env!("CARGO_PKG_VERSION")));
            // macOS: a menu bar app, without a Dock icon or an app menu.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            overlay::create(app.handle())?;
            webview_memory::awake_changed(app.handle(), false);
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
            if let Some(path) = klat_protocol::runtime_path() {
                event_server::remove_runtime(&path);
            }
            log::info("exiting");
        }
    });
}

fn start_event_server(app: &AppHandle) -> std::io::Result<()> {
    // Without a data folder there is nowhere private to hand the token to the hooks.
    let Some(runtime) = klat_protocol::runtime_path() else {
        log::warn("no data folder (APPDATA / HOME unset): hook events are off");
        return Ok(());
    };
    let token = event_server::new_token()?;
    let handle = app.clone();
    let port = event_server::start(token.clone(), move |ev| {
        state::apply_hook_event(&handle, &ev)
    })?;
    log::info(format!("event server listening on 127.0.0.1:{port}"));
    event_server::write_runtime(
        &runtime,
        &klat_protocol::RuntimeInfo {
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
