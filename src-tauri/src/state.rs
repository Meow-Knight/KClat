//! Shared app state and the `cat://state` / `cat://settings` events.
//!
//! The cat is awake when a Claude Code session needs you (session store, spec §8.2) or after the tray's
//! "Wake test". Hook events arrive through the event server and are applied here.
//!
//! Ready: by default a finished turn does not wake the cat (you are probably watching Claude). Arming Ready
//! ("sleep now, wake me when Claude is done") lets finished turns wake it. Ready stays armed until you turn it
//! off with the bell button or the tray item. Permission and input waits always wake.

use crate::cursor::Rect;
use crate::log;
use crate::session_store::{Gate, HookEvent, Outcome, SessionStore};
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

/// Why the last change happened, so the overlay can pick how the cat falls asleep: knocked out by a flying
/// object when you did something (answered Claude, dismissed, turned Ready off), a yawn when nobody did
/// (a waiting session expired, notifications were paused).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SleepCause {
    User,
    Auto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatState {
    pub awake: bool,
    pub reason: Option<WakeReason>,
    /// Sessions currently calling for you, oldest first.
    pub sessions: Vec<SessionView>,
    pub attention: Option<Attention>,
    /// Ready is armed: finished turns wake the cat until you turn it off.
    pub ready: bool,
    /// Why the latest change happened; read by the overlay when the cat goes from awake to asleep.
    pub sleep_cause: SleepCause,
}

struct Inner {
    store: SessionStore,
    /// Set by the tray's "Wake test"; cleared by Sleep / Dismiss.
    test_wake: Option<WakeReason>,
    attention: Option<Attention>,
    /// When Ready was armed (unix ms); None while it is off. Only finished turns that start after it count.
    ready_since: Option<u64>,
    sleep_cause: SleepCause,
}

impl Inner {
    /// Ready starts armed: by default the cat wakes when Claude finishes. It is not saved, so turning it off lasts
    /// until the next launch.
    fn at_launch() -> Self {
        Self {
            store: SessionStore::default(),
            test_wake: None,
            attention: None,
            ready_since: Some(now_ms()),
            sleep_cause: SleepCause::Auto,
        }
    }
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_path: PathBuf,
    inner: Mutex<Inner>,
    /// Clickable regions of the overlay as the page reports them (CSS px); see `hitboxes()`.
    pub page_hitboxes: Mutex<Vec<Rect>>,
    /// Mirrors the last published `awake`, read lock-free by the cursor poller.
    pub awake: AtomicBool,
}

/// The data folder (settings, runtime.json, log): see data_dir.rs. The temp folder if the base is unknown.
pub fn data_dir() -> PathBuf {
    klat_protocol::data_dir().unwrap_or_else(|| std::env::temp_dir().join("Klat"))
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
            inner: Mutex::new(Inner::at_launch()),
            page_hitboxes: Mutex::new(Vec::new()),
            awake: AtomicBool::new(false),
        }
    }

    fn gate(&self, inner: &Inner) -> Gate {
        Gate {
            paused: !self.settings.lock().unwrap().notifications_enabled,
            ready_since: inner.ready_since,
        }
    }

    pub fn snapshot(&self) -> CatState {
        let inner = self.inner.lock().unwrap();
        self.snapshot_of(&inner)
    }

    fn snapshot_of(&self, inner: &Inner) -> CatState {
        let gate = self.gate(inner);
        let sessions = inner.store.waiting(gate);
        let awake = inner.test_wake.is_some() || inner.store.awake(gate);
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
            ready: inner.ready_since.is_some(),
            sleep_cause: inner.sleep_cause,
        }
    }

    /// Clickable regions in logical px relative to the overlay's top-left corner.
    pub fn hitboxes(&self) -> Vec<Rect> {
        let zoom = crate::overlay::zoom();
        let rects = self.page_hitboxes.lock().unwrap();
        rects.iter().map(|r| r.scaled(zoom)).collect()
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

/// The Ready button next to the cat, or the tray item.
pub fn set_ready(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.sleep_cause = SleepCause::User;
        // Arming again while already armed keeps the original time, so nothing that already counts is lost.
        inner.ready_since = if on {
            inner.ready_since.or(Some(now_ms()))
        } else {
            None
        };
    }
    log::info(format!("ready {}", if on { "armed" } else { "off" }));
    publish_state(app);
}

/// Sleep, Dismiss all and a bubble click all end up here. Never sends anything to Claude Code.
pub fn dismiss_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut inner = state.inner.lock().unwrap();
        inner.sleep_cause = SleepCause::User;
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
        // Hooks follow what you do in Claude Code: a prompt, an answered permission, a closed session.
        inner.sleep_cause = SleepCause::User;
        let outcome = inner.store.apply(ev, now_ms());
        if let Outcome::Attention {
            project_name,
            reason,
        } = &outcome
        {
            // Only attention that may wake the cat makes it yell again; a finished turn without Ready does not.
            let gate = state.gate(&inner);
            if inner.store.calls_for_you(&ev.session_id, gate) {
                bump_attention(&mut inner, project_name.clone(), *reason);
            }
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
    let changed = {
        let mut inner = state.inner.lock().unwrap();
        let changed = inner.store.expire(now_ms(), stale_after);
        if changed {
            inner.sleep_cause = SleepCause::Auto;
        }
        changed
    };
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
        log::warn(format!("could not save settings: {e}"));
    }
    // A settings change that puts the cat to sleep is Pause: nobody to throw anything.
    state.inner.lock().unwrap().sleep_cause = SleepCause::Auto;
    crate::tray::sync_settings(app, &next);
    crate::settings_window::sync_title(app, next.ui_language);
    let _ = app.emit(EVENT_SETTINGS, next.clone());
    // Pausing or resuming notifications changes whether the cat is awake.
    publish_state(app);
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sleep_cause_uses_the_names_the_overlay_reads() {
        assert_eq!(serde_json::to_value(SleepCause::User).unwrap(), "user");
        assert_eq!(serde_json::to_value(SleepCause::Auto).unwrap(), "auto");
    }

    #[test]
    fn the_snapshot_carries_the_cause() {
        let state = AppState {
            settings: Mutex::new(Settings::default()),
            settings_path: PathBuf::new(),
            inner: Mutex::new(Inner {
                store: SessionStore::default(),
                test_wake: None,
                attention: None,
                ready_since: None,
                sleep_cause: SleepCause::User,
            }),
            page_hitboxes: Mutex::new(Vec::new()),
            awake: AtomicBool::new(false),
        };
        let json = serde_json::to_value(state.snapshot()).unwrap();
        assert_eq!(json["sleepCause"], "user");
        assert_eq!(json["awake"], false);
    }

    #[test]
    fn ready_is_armed_at_launch() {
        assert!(Inner::at_launch().ready_since.is_some());
    }
}
