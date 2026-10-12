//! Shared app state and the `cat://state` / `cat://settings` events.
//!
//! The cat is awake when a Claude Code session needs you (session store, spec §8.2) or after the tray's
//! "Wake test". Hook events arrive through the event server and are applied here.

use crate::cursor::Rect;
use crate::log;
use crate::session_store::{HookEvent, Outcome, SessionStore};
pub use crate::session_store::{SessionView, WakeReason};
use crate::settings::{self, Settings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_STATE: &str = "cat://state";
pub const EVENT_SETTINGS: &str = "cat://settings";

/// The latest "a session needs you" moment. `seq` changes on each one, so the overlay can tell a new
/// session (or a new reason) that arrives while the cat is already awake and yell again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    pub seq: u64,
    pub project_name: String,
    pub reason: WakeReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatState {
    pub awake: bool,
    pub reason: Option<WakeReason>,
    /// Sessions currently calling for you, oldest first.
    pub sessions: Vec<SessionView>,
    pub attention: Option<Attention>,
}

struct Inner {
    store: SessionStore,
    /// Set by the tray's "Wake test"; cleared by Sleep / Dismiss.
    test_wake: Option<WakeReason>,
    attention: Option<Attention>,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    inner: Mutex<Inner>,
    /// Clickable regions of the overlay in logical px, reported by the frontend.
    pub hitboxes: Mutex<Vec<Rect>>,
    /// Mirrors the last published `awake`, read lock-free by the cursor poller.
    pub awake: AtomicBool,
}

/// `%APPDATA%\Klat` (settings, runtime.json, log).
pub fn data_dir() -> PathBuf {
    settings::default_dir().unwrap_or_else(|| std::env::temp_dir().join("Klat"))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl AppState {
    pub fn load() -> Self {
        let settings_path = data_dir().join("settings.json");
        let (settings, outcome) = settings::load(&settings_path);
        if let settings::LoadOutcome::RecoveredCorrupt(moved) = &outcome {
            log::warn(format!(
                "settings were corrupt; moved to {} and reset to defaults",
                moved.display()
            ));
        }
        Self {
            settings: Mutex::new(settings),
            settings_path,
            inner: Mutex::new(Inner {
                store: SessionStore::default(),
                test_wake: None,
                attention: None,
            }),
            hitboxes: Mutex::new(Vec::new()),
            awake: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> CatState {
        let paused = !self.settings.lock().unwrap().notifications_enabled;
        let inner = self.inner.lock().unwrap();
        let sessions = if paused {
            Vec::new()
        } else {
            inner.store.waiting()
        };
        let awake = inner.test_wake.is_some() || inner.store.awake(paused);
        let reason = if !awake {
            None
        } else {
            sessions.last().map(|s| s.reason).or(inner.test_wake)
        };
        CatState {
            awake,
            reason,
            sessions,
            attention: if awake { inner.attention.clone() } else { None },
        }
    }

    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }
}

pub fn wake_test(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.test_wake = Some(WakeReason::Done);
        bump_attention(&mut inner, String::new(), WakeReason::Done);
    }
    publish_state(app);
}

/// Sleep, Dismiss all and a bubble click all end up here. Never sends anything to Claude Code.
pub fn dismiss_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.test_wake = None;
        inner.store.dismiss_all();
    }
    publish_state(app);
}

/// A validated hook event from the event server.
pub fn apply_hook_event(app: &AppHandle, ev: &HookEvent) {
    let state = app.state::<AppState>();
    let outcome = {
        let mut inner = state.inner.lock().unwrap();
        let outcome = inner.store.apply(ev, now_ms());
        if let Outcome::Attention {
            project_name,
            reason,
        } = &outcome
        {
            bump_attention(&mut inner, project_name.clone(), *reason);
        }
        outcome
    };
    // Only names and outcomes: the adapter never forwards prompts, commands or tool input.
    let session: String = ev.session_id.chars().take(8).collect();
    log::info(format!(
        "hook {}{} session {session} project {:?} -> {}",
        ev.hook_event_name,
        ev.notification_type
            .as_deref()
            .map(|t| format!("({t})"))
            .unwrap_or_default(),
        ev.project_name.as_deref().unwrap_or(""),
        match &outcome {
            Outcome::Unchanged => "ignored".to_owned(),
            Outcome::Changed => "cleared".to_owned(),
            Outcome::Attention { reason, .. } => format!("attention {reason:?}"),
        }
    ));
    if outcome != Outcome::Unchanged {
        publish_state(app);
    }
}

/// Clears sessions that have waited longer than `staleAfterMs`.
pub fn expire_stale(app: &AppHandle) {
    let state = app.state::<AppState>();
    let stale_after = state.settings.lock().unwrap().stale_after_ms;
    let changed = state
        .inner
        .lock()
        .unwrap()
        .store
        .expire(now_ms(), stale_after);
    if changed {
        publish_state(app);
    }
}

fn bump_attention(inner: &mut Inner, project_name: String, reason: WakeReason) {
    let seq = inner.attention.as_ref().map_or(1, |a| a.seq + 1);
    inner.attention = Some(Attention {
        seq,
        project_name,
        reason,
    });
}

pub fn publish_state(app: &AppHandle) {
    let state = app.state::<AppState>();
    let snapshot = state.snapshot();
    state.awake.store(snapshot.awake, Ordering::Relaxed);
    crate::tray::set_waiting(app, snapshot.sessions.len());
    let _ = app.emit(EVENT_STATE, snapshot);
}

/// Applies a change, saves the file, and pushes the new settings to the overlay.
pub fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
    let state = app.state::<AppState>();
    let next = {
        let mut s = state.settings.lock().unwrap();
        change(&mut s);
        s.clone()
    };
    if let Err(e) = settings::save(&state.settings_path, &next) {
        log::warn(format!("could not save settings: {e}"));
    }
    let _ = app.emit(EVENT_SETTINGS, next);
    // Pausing or resuming notifications changes whether the cat is awake.
    publish_state(app);
}
