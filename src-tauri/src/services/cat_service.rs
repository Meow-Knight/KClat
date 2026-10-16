//! The cat's state and the settings: their one owner (docs/ARCHITECTURE.md §5.4, §6).
//!
//! The rules are klat_core::cat and klat_core::settings; this holds the lock, reads the clock, saves the settings
//! file, logs, and publishes on two `watch` channels: the snapshot (`cat://state`) and the settings. Whatever
//! reacts to a change (the tray, the webviews, the webview memory level) subscribes; nothing here knows about it,
//! and nothing here needs Tauri.

use klat_core::cat::{CatModel, CatState, Command, Ctx, Handled};
use klat_core::session::Outcome;
use klat_core::settings::Settings;
use klat_core::time::UnixMs;
use klat_infra::settings_repo;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::watch;

/// The clock, read here and passed into klat-core, which never reads it.
pub fn now_ms() -> UnixMs {
    UnixMs(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
    )
}

/// The cat and the settings behind one lock, so a settings change and what it does to the cat (Pause puts it to
/// sleep, with the cause "auto") are one step that no one sees half done.
struct Model {
    cat: CatModel,
    settings: Settings,
}

impl Model {
    fn paused(&self) -> bool {
        !self.settings.notifications_enabled
    }
}

pub struct CatService {
    /// The only lock. Held while a command runs and while its results are sent, so snapshots go out in order.
    /// Never held while calling anything outside this file: subscribers only read the channels.
    model: Mutex<Model>,
    settings_path: PathBuf,
    clock: Box<dyn Fn() -> UnixMs + Send + Sync>,
    state: watch::Sender<CatState>,
    settings: watch::Sender<Settings>,
}

impl CatService {
    /// `settings` as loaded from `settings_path`, where changes are saved.
    pub fn new(
        settings: Settings,
        settings_path: PathBuf,
        clock: impl Fn() -> UnixMs + Send + Sync + 'static,
    ) -> Self {
        let model = Model {
            cat: CatModel::at_launch(clock()),
            settings,
        };
        let snapshot = model.cat.snapshot(model.paused());
        Self {
            state: watch::Sender::new(snapshot),
            settings: watch::Sender::new(model.settings.clone()),
            model: Mutex::new(model),
            settings_path,
            clock: Box::new(clock),
        }
    }

    /// Runs a command against the cat's state, with the time and Pause read now, and publishes the snapshot if
    /// it changed.
    pub fn dispatch(&self, cmd: Command<'_>) -> Handled {
        let hook = match cmd {
            Command::Hook(ev) => Some(ev),
            _ => None,
        };
        let ready = match cmd {
            Command::SetReady(on) => Some(on),
            _ => None,
        };
        let handled = self.run(&mut self.model.lock().unwrap(), cmd);
        if let Some(ev) = hook {
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
        }
        if let Some(on) = ready {
            log::info!("ready {}", if on { "armed" } else { "off" });
        }
        handled
    }

    /// Applies a change, saves the file, and lets the cat follow (Pause), all under the lock. Returns the settings
    /// now in effect.
    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) -> Settings {
        let mut model = self.model.lock().unwrap();
        change(&mut model.settings);
        let next = model.settings.clone();
        if let Err(e) = settings_repo::save(&self.settings_path, &next) {
            log::warn!("could not save settings: {e}");
        }
        self.settings.send_if_modified(|current| {
            let modified = *current != next;
            if modified {
                *current = next.clone();
            }
            modified
        });
        self.run(&mut model, Command::SettingsChanged);
        next
    }

    fn run(&self, model: &mut Model, cmd: Command<'_>) -> Handled {
        let ctx = Ctx {
            now: (self.clock)(),
            paused: model.paused(),
        };
        let handled = model.cat.handle(cmd, ctx);
        let next = model.cat.snapshot(model.paused());
        // Always kept current, so `snapshot()` is what the model says; subscribers are told only when klat-core
        // says to publish (a hook it ignored still records that you are at the keyboard, quietly).
        self.state.send_if_modified(|current| {
            if *current == next {
                return false;
            }
            *current = next;
            handled.publish
        });
        handled
    }

    pub fn snapshot(&self) -> CatState {
        self.state.borrow().clone()
    }

    pub fn settings(&self) -> Settings {
        self.settings.borrow().clone()
    }

    /// Told about every snapshot published from now on (not the current one).
    pub fn subscribe(&self) -> watch::Receiver<CatState> {
        self.state.subscribe()
    }

    /// Told about every settings change from now on (not the current settings).
    pub fn subscribe_settings(&self) -> watch::Receiver<Settings> {
        self.settings.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klat_core::cat::SleepCause;
    use klat_core::session::WakeReason;
    use klat_core::time::DurationMs;
    use klat_protocol::HookEvent;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct Fixture {
        svc: CatService,
        time: Arc<AtomicU64>,
        _dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new(settings: Settings) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let time = Arc::new(AtomicU64::new(1_000));
            let clock = {
                let time = time.clone();
                move || UnixMs(time.load(Ordering::SeqCst))
            };
            let svc = CatService::new(settings, dir.path().join("settings.json"), clock);
            Self {
                svc,
                time,
                _dir: dir,
            }
        }

        fn at(&self, ms: u64) -> &CatService {
            self.time.store(ms, Ordering::SeqCst);
            &self.svc
        }
    }

    fn hook(name: &str, session: &str) -> HookEvent {
        HookEvent {
            event_id: "e".into(),
            hook_event_name: name.into(),
            notification_type: None,
            session_id: session.into(),
            project_name: Some(format!("proj-{session}")),
            tool_name: None,
            occurred_at: 0,
        }
    }

    /// The new value if subscribers were told about one since the last call.
    fn seen<T: Clone>(rx: &mut watch::Receiver<T>) -> Option<T> {
        rx.has_changed()
            .unwrap()
            .then(|| rx.borrow_and_update().clone())
    }

    #[test]
    fn a_hook_that_calls_for_you_reaches_subscribers() {
        let f = Fixture::new(Settings::default());
        let mut rx = f.svc.subscribe();
        f.at(2_000).dispatch(Command::Hook(&hook("Stop", "a")));
        let s = seen(&mut rx).expect("published");
        assert!(s.awake);
        assert_eq!(s.reason, Some(WakeReason::Done));
        assert!(f.svc.snapshot().awake);

        f.at(3_000)
            .dispatch(Command::Hook(&hook("PermissionRequest", "b")));
        let s = seen(&mut rx).expect("published");
        assert_eq!(s.reason, Some(WakeReason::Permission));
        assert_eq!(s.sessions.len(), 2);
    }

    #[test]
    fn commands_set_the_sleep_cause() {
        let f = Fixture::new(Settings::default());
        let mut rx = f.svc.subscribe();
        let cause = |rx: &mut watch::Receiver<CatState>| seen(rx).expect("published").sleep_cause;

        f.at(2_000).dispatch(Command::SetReady(false));
        assert_eq!(cause(&mut rx), SleepCause::User);

        f.at(3_000)
            .update_settings(|s| s.notifications_enabled = false);
        assert_eq!(cause(&mut rx), SleepCause::Auto);
        f.at(3_500)
            .update_settings(|s| s.notifications_enabled = true);

        f.at(4_000)
            .dispatch(Command::Hook(&hook("PermissionRequest", "a")));
        f.at(5_000).dispatch(Command::DismissAll);
        assert_eq!(cause(&mut rx), SleepCause::User);

        f.at(6_000)
            .dispatch(Command::Hook(&hook("PermissionRequest", "b")));
        let stale = Command::ExpireStale {
            stale_after: DurationMs(1_000),
        };
        f.at(9_000).dispatch(stale);
        let s = seen(&mut rx).expect("published");
        assert!(!s.awake);
        assert_eq!(s.sleep_cause, SleepCause::Auto);
    }

    #[test]
    fn pausing_saves_and_puts_the_cat_to_sleep_in_one_step() {
        let f = Fixture::new(Settings::default());
        f.at(2_000)
            .dispatch(Command::Hook(&hook("PermissionRequest", "a")));
        assert!(f.svc.snapshot().awake);
        let mut state = f.svc.subscribe();
        let mut settings = f.svc.subscribe_settings();

        let next = f
            .at(3_000)
            .update_settings(|s| s.notifications_enabled = false);
        // By the time it returns, all three agree: the settings, the file and the cat.
        assert!(!next.notifications_enabled);
        assert_eq!(seen(&mut settings), Some(next.clone()));
        let s = seen(&mut state).expect("published");
        assert!(!s.awake);
        assert_eq!(s.sleep_cause, SleepCause::Auto);
        let (on_disk, _) = settings_repo::load(&f.svc.settings_path);
        assert_eq!(on_disk, next);

        // Resuming wakes it again for the wait that Pause held.
        f.at(4_000)
            .update_settings(|s| s.notifications_enabled = true);
        let s = seen(&mut state).expect("published");
        assert!(s.awake);
        assert_eq!(s.sleep_cause, SleepCause::Auto);
    }

    #[test]
    fn subscribers_are_not_told_when_nothing_changed() {
        let f = Fixture::new(Settings::default());
        let mut state = f.svc.subscribe();
        let mut settings = f.svc.subscribe_settings();
        f.at(2_000).dispatch(Command::DismissAll);
        assert!(seen(&mut state).is_some(), "the cause went auto -> user");
        f.at(3_000).dispatch(Command::DismissAll);
        assert_eq!(seen(&mut state), None);
        f.at(4_000).update_settings(|_| {});
        assert_eq!(seen(&mut settings), None);
    }

    #[test]
    fn an_ignored_hook_is_recorded_without_telling_anyone() {
        let f = Fixture::new(Settings::default());
        let mut rx = f.svc.subscribe();
        // UserPromptSubmit with nothing waiting: klat-core says "unchanged", yet you are at the keyboard.
        let handled = f
            .at(2_000)
            .dispatch(Command::Hook(&hook("UserPromptSubmit", "a")));
        assert!(!handled.publish);
        assert_eq!(seen(&mut rx), None);
        assert_eq!(f.svc.snapshot().sleep_cause, SleepCause::User);
    }
}
