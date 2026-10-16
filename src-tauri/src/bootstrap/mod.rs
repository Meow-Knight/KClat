//! Composition root: the Tauri builder, plugins, managed state, setup and shutdown (docs/ARCHITECTURE.md §5.4).

mod fanout;

use crate::ipc::commands;
use crate::platform;
use crate::services::cat_service::{self, CatService};
use crate::services::settings_service;
use crate::tasks::{cursor_poller, drag_loop, stale_ticker};
use crate::ui::overlay::{self, OverlayState};
use crate::ui::{settings_window, tray};
use klat_core::cat::Command;
use klat_infra::{event_server, runtime_file, settings_repo};
use std::sync::Arc;
use tauri::{Manager, RunEvent};

pub fn run() {
    settings_repo::migrate_legacy_dir();
    let data_dir = settings_service::data_dir();
    klat_infra::log::init(&data_dir);
    let (settings, settings_path) = settings_service::load(&data_dir);
    let cat = Arc::new(CatService::new(
        settings,
        settings_path,
        cat_service::now_ms,
    ));
    let overlay_state = Arc::new(OverlayState::default());

    let app = tauri::Builder::default()
        // Must come first: a second launch hands over to this instance and exits before creating anything.
        .plugin(tauri_plugin_single_instance::init({
            let cat = cat.clone();
            move |app, _args, _cwd| {
                log::info!("second launch ignored: already running");
                tray::notify(
                    app,
                    klat_core::i18n::t(cat.settings().ui_language, "dialog.alreadyRunning"),
                );
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(cat.clone())
        .manage(overlay_state.clone())
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
        .on_window_event({
            let (cat, overlay_state) = (cat.clone(), overlay_state.clone());
            move |window, event| match event {
                tauri::WindowEvent::ScaleFactorChanged { .. } => {
                    if let Some(win) = overlay::get(window.app_handle()) {
                        overlay::on_scale_changed(&win, cat.clone(), overlay_state.clone());
                    }
                }
                // Closed in the middle of a size slider drag: put the cat back at its saved size.
                tauri::WindowEvent::Destroyed if window.label() == settings_window::LABEL => {
                    overlay::end_preview(window.app_handle());
                }
                _ => {}
            }
        })
        .setup(move |app| {
            // Logged here, not before the builder: a second launch exits inside the single-instance plugin.
            log::info!("Klat {} starting", env!("CARGO_PKG_VERSION"));
            platform::menu_bar_app(app);
            fanout::start(app.handle(), &cat);
            overlay::create(app.handle(), &cat)?;
            platform::webview_memory::awake_changed(app.handle(), false);
            settings_service::reconcile_start_at_login(app.handle(), &cat);
            tray::create(app.handle(), cat.clone())?;
            cursor_poller::start(app.handle().clone(), cat.clone(), overlay_state.clone());
            drag_loop::start(app.handle().clone(), cat.clone(), overlay_state);
            start_event_server(cat.clone())?;
            stale_ticker::start(cat);
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

fn start_event_server(cat: Arc<CatService>) -> std::io::Result<()> {
    // Without a data folder there is nowhere private to hand the token to the hooks.
    let Some(runtime) = klat_protocol::runtime_path() else {
        log::warn!("no data folder (APPDATA / HOME unset): hook events are off");
        return Ok(());
    };
    let token = event_server::new_token()?;
    let port = event_server::start(token.clone(), cat_service::now_ms, move |ev| {
        cat.dispatch(Command::Hook(&ev));
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
