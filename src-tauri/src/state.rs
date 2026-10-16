//! Shared app state and the `cat://state` / `cat://settings` events.
//!
//! The rules (when the cat is awake, Ready, Pause, Dismiss) are klat_core::cat; this holds the lock, reads the
//! clock and the settings, logs, and publishes the snapshot.

// target: services::cat_service

use crate::settings;
pub use klat_core::cat::CatState;
use klat_core::cat::{CatModel, Command, Ctx, Handled};
use klat_core::geometry::{Css, Logical, Rect};
use klat_core::session::Outcome;
use klat_core::settings::Settings;
use klat_core::time::UnixMs;
use klat_protocol::HookEvent;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_STATE: &str = "cat://state";
pub const EVENT_SETTINGS: &str = "cat://settings";

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    cat: Mutex<CatModel>,
    /// Clickable regions of the overlay as the page reports them; see `hitboxes()`.
    pub page_hitboxes: Mutex<Vec<Rect<Css>>>,
    /// Mirrors the last published `awake`, read lock-free by the cursor poller.
    pub awake: AtomicBool,
}

/// The data folder (settings, runtime.json, log): see data_dir.rs. The temp folder if the base is unknown.
pub fn data_dir() -> PathBuf {
    klat_protocol::data_dir().unwrap_or_else(|| std::env::temp_dir().join("Klat"))
}

/// The clock, read here and passed into klat-core, which never reads it.
pub fn now_ms() -> UnixMs {
    UnixMs(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    )
}

impl AppState {
    pub fn load() -> Self {
        let settings_path = data_dir().join("settings.json");
        let (settings, outcome) = settings::load(&settings_path);
        if let settings::LoadOutcome::RecoveredCorrupt(moved) = &outcome {
            log::warn!(
                "settings were corrupt; moved to {} and reset to defaults",
                moved.display()
            );
        }
        Self {
            settings: Mutex::new(settings),
            settings_path,
            cat: Mutex::new(CatModel::at_launch(now_ms())),
            page_hitboxes: Mutex::new(Vec::new()),
            awake: AtomicBool::new(false),
        }
    }

    fn paused(&self) -> bool {
        !self.settings.lock().unwrap().notifications_enabled
    }

    pub fn snapshot(&self) -> CatState {
        let paused = self.paused();
        self.cat.lock().unwrap().snapshot(paused)
    }

    /// Clickable regions relative to the overlay's top-left corner.
    pub fn hitboxes(&self) -> Vec<Rect<Logical>> {
        let zoom = crate::overlay::zoom();
        let rects = self.page_hitboxes.lock().unwrap();
        rects.iter().map(|r| r.to_logical(zoom)).collect()
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

/// Runs a command against the cat's state, with the time and Pause read now.
fn handle(app: &AppHandle, cmd: Command<'_>) -> Handled {
    let state = app.state::<AppState>();
    let ctx = Ctx {
        now: now_ms(),
        paused: state.paused(),
    };
    let mut cat = state.cat.lock().unwrap();
    cat.handle(cmd, ctx)
}

pub fn wake_test(app: &AppHandle) {
    handle(app, Command::WakeTest);
    publish_state(app);
}

/// The Ready button next to the cat, or the tray item.
pub fn set_ready(app: &AppHandle, on: bool) {
    handle(app, Command::SetReady(on));
    log::info!("ready {}", if on { "armed" } else { "off" });
    publish_state(app);
}

/// Sleep, Dismiss all and a bubble click all end up here. Never sends anything to Claude Code.
pub fn dismiss_all(app: &AppHandle) {
    handle(app, Command::DismissAll);
    publish_state(app);
}

/// A validated hook event from the event server.
pub fn apply_hook_event(app: &AppHandle, ev: &HookEvent) {
    let handled = handle(app, Command::Hook(ev));
    // Only names and outcomes: the adapter never forwards prompts, commands or tool input.
    let session: String = ev.session_id.chars().take(8).collect();
    log::info!(
        "hook {}{} session {session} project {:?} -> {}",
        ev.hook_event_name,
        ev.notification_type
            .as_deref()
            .map(|t| format!("({t})"))
            .unwrap_or_default(),
        ev.project_name.as_deref().unwrap_or(""),
        match &handled.hook {
            Some(Outcome::Unchanged) | None => "ignored".to_owned(),
            Some(Outcome::Changed) => "cleared".to_owned(),
            Some(Outcome::Attention { reason, .. }) => format!("attention {reason:?}"),
        }
    );
    if handled.publish {
        publish_state(app);
    }
}

/// Clears sessions that have waited longer than `staleAfterMs`.
pub fn expire_stale(app: &AppHandle) {
    let stale_after = app.state::<AppState>().settings().stale_after_ms;
    if handle(app, Command::ExpireStale { stale_after }).publish {
        publish_state(app);
    }
}

pub fn publish_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot();
    if state.awake.swap(snapshot.awake, Ordering::Relaxed) != snapshot.awake {
        crate::webview_memory::awake_changed(app, snapshot.awake);
    }
    crate::tray::set_waiting(app, snapshot.sessions.len());
    crate::tray::set_ready_checked(snapshot.ready);
    let _ = app.emit(EVENT_STATE, snapshot);
}

/// Applies a change, saves the file, and pushes the new settings to the overlay, the Settings window and the
/// tray ticks. Returns the settings now in effect.
pub fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> Settings {
    let state = app.state::<AppState>();
    let next = {
        let mut s = state.settings.lock().unwrap();
        change(&mut s);
        s.clone()
    };
    if let Err(e) = settings::save(&state.settings_path, &next) {
        log::warn!("could not save settings: {e}");
    }
    handle(app, Command::SettingsChanged);
    crate::tray::sync_settings(app, &next);
    crate::settings_window::sync_title(app, next.ui_language);
    let _ = app.emit(EVENT_SETTINGS, next.clone());
    // Pausing or resuming notifications changes whether the cat is awake.
    publish_state(app);
    next
}
