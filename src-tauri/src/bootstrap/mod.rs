//! Composition root: the Tauri builder, plugins, managed state, setup and shutdown (docs/ARCHITECTURE.md §5.4).

use crate::ipc::commands;
use crate::platform;
use crate::services::{autostart, cat_service, i18n};
use crate::tasks::{cursor_poller, drag_loop, stale_ticker};
use crate::ui::{overlay, settings_window, tray};
use klat_infra::{event_server, runtime_file, settings_repo};
use tauri::{AppHandle, Manager, RunEvent};

pub fn run() {
    settings_repo::migrate_legacy_dir();
    klat_infra::log::init(&cat_service::data_dir());

    let app = tauri::Builder::default()
        // Must come first: a second launch hands over to this instance and exits before creating anything.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            log::info!("second launch ignored: already running");
            tray::notify(
                app,
                klat_core::i18n::t(i18n::current(app), "dialog.alreadyRunning"),
            );
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(cat_service::AppState::load())
        .invoke_handler(tauri::generate_handler![
            commands::cat::get_state,
            commands::settings::get_settings,
            commands::cat::dismiss,
            commands::overlay::set_hitboxes,
            commands::cat::set_ready,
            commands::settings::set_settings,
            commands::overlay::preview_cat_scale,
            commands::overlay::overlay_preview_zoom,
            commands::settings::set_start_at_login,
            commands::cat::wake_test,
            commands::cat::sleep,
            commands::hooks::hooks_status,
            commands::hooks::hooks_apply,
            commands::settings::open_settings,
            commands::drag::drag_permission,
            commands::drag::open_drag_permission,
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
            log::info!("Klat {} starting", env!("CARGO_PKG_VERSION"));
            platform::menu_bar_app(app);
            overlay::create(app.handle())?;
            platform::webview_memory::awake_changed(app.handle(), false);
            autostart::reconcile(app.handle());
            tray::create(app.handle())?;
            cursor_poller::start(app.handle().clone());
            drag_loop::start(app.handle().clone());
            start_event_server(app.handle())?;
            stale_ticker::start(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building Klat");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            if let Some(path) = klat_protocol::runtime_path() {
                runtime_file::remove_runtime(&path);
            }
            log::info!("exiting");
        }
    });
}

fn start_event_server(app: &AppHandle) -> std::io::Result<()> {
    // Without a data folder there is nowhere private to hand the token to the hooks.
    let Some(runtime) = klat_protocol::runtime_path() else {
        log::warn!("no data folder (APPDATA / HOME unset): hook events are off");
        return Ok(());
    };
    let token = event_server::new_token()?;
    let handle = app.clone();
    let port = event_server::start(token.clone(), cat_service::now_ms, move |ev| {
        cat_service::apply_hook_event(&handle, &ev)
    })?;
    log::info!("event server listening on 127.0.0.1:{port}");
    runtime_file::write_runtime(
        &runtime,
        &klat_protocol::RuntimeInfo {
            port,
            token,
            pid: std::process::id(),
        },
    )
}
