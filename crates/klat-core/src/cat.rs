//! When the cat is awake, and why (spec §8.2): the session store plus Ready, Pause, Dismiss and the Wake test.
//! A reducer: [`CatModel::handle`] applies one [`Command`] at the time and Pause given in [`Ctx`], and
//! [`CatModel::snapshot`] is what the app publishes as `cat://state`. The app holds the lock, reads the clock
//! and the settings, logs and publishes; every rule is here.
//!
//! Ready: by default a finished turn does not wake the cat (you are probably watching Claude). Arming Ready
//! ("sleep now, wake me when Claude is done") lets finished turns wake it. Ready stays armed until you turn it
//! off with the bell button or the tray item. Permission and input waits always wake.

use crate::session::{Gate, Outcome, SessionStore, SessionView, WakeReason};
use crate::time::{DurationMs, UnixMs};
use klat_protocol::HookEvent;
use serde::Serialize;

/// The latest "a session needs you" moment. `seq` changes on each one, so the overlay can tell a new
/// session (or a new reason) that arrives while the cat is already awake and yell again.
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attention {
    #[cfg_attr(feature = "ts", ts(type = "number"))]
    pub seq: u64,
    pub project_name: String,
    pub reason: WakeReason,
}

/// Why the last change happened, so the overlay can pick how the cat falls asleep: knocked out by a flying
/// object when you did something (answered Claude, dismissed, turned Ready off), a yawn when nobody did
/// (a waiting session expired, notifications were paused).
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SleepCause {
    User,
    Auto,
}

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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

/// The cat's state. Every change goes through [`CatModel::handle`]; what it looks like from outside is
/// [`CatModel::snapshot`].
#[derive(Debug)]
pub struct CatModel {
    store: SessionStore,
    /// Set by the tray's "Wake test"; cleared by Sleep / Dismiss.
    test_wake: Option<WakeReason>,
    attention: Option<Attention>,
    /// When Ready was armed; None while it is off. Only finished turns that start after it count.
    ready_since: Option<UnixMs>,
    sleep_cause: SleepCause,
}

/// Something that may change whether the cat is awake.
pub enum Command<'a> {
    /// A validated hook event from the event server.
    Hook(&'a HookEvent),
    /// The Ready button next to the cat, or the tray item.
    SetReady(bool),
    /// Sleep, Dismiss all and a bubble click. Never sends anything to Claude Code.
    DismissAll,
    /// The tray's "Wake test".
    WakeTest,
    /// Clears sessions that have waited longer than `stale_after` (the `staleAfterMs` setting).
    ExpireStale { stale_after: DurationMs },
    /// The settings changed (Pause among them).
    SettingsChanged,
}

/// What a command needs to know about the world: the time, and whether notifications are paused.
#[derive(Debug, Clone, Copy)]
pub struct Ctx {
    pub now: UnixMs,
    pub paused: bool,
}

/// What a command did, for the caller to log and publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handled {
    /// The snapshot may have changed: publish it.
    pub publish: bool,
    /// For a hook: what the session store made of it.
    pub hook: Option<Outcome>,
}

impl CatModel {
    /// Ready starts armed: by default the cat wakes when Claude finishes. It is not saved, so turning it off lasts
    /// until the next launch.
    pub fn at_launch(now: UnixMs) -> Self {
        Self {
            store: SessionStore::default(),
            test_wake: None,
            attention: None,
            ready_since: Some(now),
            sleep_cause: SleepCause::Auto,
        }
    }

    fn gate(&self, paused: bool) -> Gate {
        Gate {
            paused,
            ready_since: self.ready_since,
        }
    }

    pub fn handle(&mut self, cmd: Command<'_>, ctx: Ctx) -> Handled {
        let changed = Handled {
            publish: true,
            hook: None,
        };
        match cmd {
            Command::WakeTest => {
                self.test_wake = Some(WakeReason::Done);
                self.bump_attention(String::new(), WakeReason::Done);
                changed
            }
            Command::SetReady(on) => {
                self.sleep_cause = SleepCause::User;
                // Arming again while already armed keeps the original time, so nothing that already counts is
                // lost.
                self.ready_since = if on {
                    self.ready_since.or(Some(ctx.now))
                } else {
                    None
                };
                changed
            }
            Command::DismissAll => {
                self.sleep_cause = SleepCause::User;
                self.test_wake = None;
                self.store.dismiss_all();
                changed
            }
            Command::Hook(ev) => {
                // Hooks follow what you do in Claude Code: a prompt, an answered permission, a closed session.
                self.sleep_cause = SleepCause::User;
                let outcome = self.store.apply(ev, ctx.now);
                if let Outcome::Attention {
                    project_name,
                    reason,
                } = &outcome
                {
                    // Only attention that may wake the cat makes it yell again; a finished turn without Ready
                    // does not.
                    let gate = self.gate(ctx.paused);
                    if self.store.calls_for_you(&ev.session_id, gate) {
                        self.bump_attention(project_name.clone(), *reason);
                    }
                }
                Handled {
                    publish: outcome != Outcome::Unchanged,
                    hook: Some(outcome),
                }
            }
            Command::ExpireStale { stale_after } => {
                let expired = self.store.expire(ctx.now, stale_after);
                if expired {
                    self.sleep_cause = SleepCause::Auto;
                }
                Handled {
                    publish: expired,
                    hook: None,
                }
            }
            Command::SettingsChanged => {
                // A settings change that puts the cat to sleep is Pause: nobody to throw anything.
                self.sleep_cause = SleepCause::Auto;
                changed
            }
        }
    }

    /// What the overlay, the tray and the Settings window are shown (`cat://state`).
    pub fn snapshot(&self, paused: bool) -> CatState {
        let gate = self.gate(paused);
        let sessions = self.store.waiting(gate);
        let awake = self.test_wake.is_some() || self.store.awake(gate);
        let reason = if !awake {
            None
        } else {
            sessions.last().map(|s| s.reason).or(self.test_wake)
        };
        CatState {
            awake,
            reason,
            sessions,
            attention: if awake { self.attention.clone() } else { None },
            ready: self.ready_since.is_some(),
            sleep_cause: self.sleep_cause,
        }
    }

    fn bump_attention(&mut self, project_name: String, reason: WakeReason) {
        let seq = self.attention.as_ref().map_or(1, |a| a.seq + 1);
        self.attention = Some(Attention {
            seq,
            project_name,
            reason,
        });
    }
}

#[cfg(test)]
mod tests {
    //! The cat's rules, one command at a time: `(state, command, ctx) -> (state', snapshot)`.

    use super::*;
    use serde_json::json;

    const T0: UnixMs = UnixMs(1_000_000);

    fn at(ms: u64) -> Ctx {
        Ctx {
            now: UnixMs(T0.0 + ms),
            paused: false,
        }
    }
    fn paused_at(ms: u64) -> Ctx {
        Ctx {
            paused: true,
            ..at(ms)
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
    fn note(kind: &str, session: &str) -> HookEvent {
        HookEvent {
            notification_type: Some(kind.into()),
            ..hook("Notification", session)
        }
    }
    /// Launched at T0 (Ready armed then).
    fn launched() -> CatModel {
        CatModel::at_launch(T0)
    }
    fn seq(m: &CatModel, paused: bool) -> Option<u64> {
        m.snapshot(paused).attention.map(|a| a.seq)
    }

    #[test]
    fn at_launch_the_cat_sleeps_with_ready_armed() {
        let s = launched().snapshot(false);
        assert!(!s.awake && s.ready);
        assert_eq!(s.sleep_cause, SleepCause::Auto);
        assert_eq!((s.reason, s.attention, s.sessions.len()), (None, None, 0));
    }

    #[test]
    fn a_finished_turn_wakes_only_when_it_ends_after_ready_was_armed() {
        let mut m = launched();
        m.handle(Command::SetReady(false), at(0));
        m.handle(Command::Hook(&hook("Stop", "a")), at(1_000));
        assert!(
            !m.snapshot(false).awake,
            "Ready off: a finished turn does not wake"
        );
        assert_eq!(seq(&m, false), None);

        m.handle(Command::SetReady(true), at(5_000));
        assert!(
            !m.snapshot(false).awake,
            "a turn that ended before arming does not count"
        );

        let handled = m.handle(Command::Hook(&hook("Stop", "b")), at(6_000));
        assert!(handled.publish);
        let s = m.snapshot(false);
        assert!(s.awake);
        assert_eq!(s.reason, Some(WakeReason::Done));
        assert_eq!(s.sessions.len(), 1);
        assert_eq!(s.attention.unwrap().project_name, "proj-b");
    }

    #[test]
    fn a_turn_finishing_at_the_moment_ready_is_armed_counts() {
        let mut m = launched();
        m.handle(Command::SetReady(false), at(0));
        m.handle(Command::SetReady(true), at(5_000));
        m.handle(Command::Hook(&hook("Stop", "a")), at(5_000));
        assert!(m.snapshot(false).awake);
    }

    #[test]
    fn permission_and_input_always_wake() {
        let mut m = launched();
        m.handle(Command::SetReady(false), at(0));
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(1_000));
        let s = m.snapshot(false);
        assert!(s.awake);
        assert_eq!(s.reason, Some(WakeReason::Permission));
        m.handle(Command::Hook(&note("idle_prompt", "b")), at(2_000));
        let s = m.snapshot(false);
        assert_eq!(
            s.reason,
            Some(WakeReason::Input),
            "the newest wait gives the reason"
        );
        assert_eq!(s.sessions.len(), 2);
    }

    #[test]
    fn pause_holds_every_wait_without_losing_it() {
        let mut m = launched();
        m.handle(
            Command::Hook(&hook("PermissionRequest", "a")),
            paused_at(1_000),
        );
        let s = m.snapshot(true);
        assert!(!s.awake);
        assert_eq!((s.reason, s.attention, s.sessions.len()), (None, None, 0));
        assert_eq!(seq(&m, false), None, "no yell is counted while paused");
        // Resuming shows the wait that arrived during the pause.
        assert!(m.snapshot(false).awake);
    }

    /// Recorded as it is: the tray's Wake test wakes the cat even while notifications are paused.
    #[test]
    fn the_wake_test_wakes_even_while_paused() {
        let mut m = launched();
        assert!(m.handle(Command::WakeTest, paused_at(0)).publish);
        let s = m.snapshot(true);
        assert!(s.awake);
        assert_eq!(s.reason, Some(WakeReason::Done));
        let a = s.attention.unwrap();
        assert_eq!((a.seq, a.project_name.as_str()), (1, ""));
    }

    #[test]
    fn dismiss_quiets_the_waits_and_ends_the_wake_test_by_your_hand() {
        let mut m = launched();
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(1_000));
        m.handle(Command::WakeTest, at(2_000));
        assert!(m.handle(Command::DismissAll, at(3_000)).publish);
        let s = m.snapshot(false);
        assert!(!s.awake);
        assert_eq!(s.sleep_cause, SleepCause::User);
        assert_eq!(s.attention, None);
        // A new request from the same session wakes it again (a Notification fallback would not: it is still
        // waiting, only dismissed).
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(9_000));
        assert!(m.snapshot(false).awake);
    }

    #[test]
    fn expiry_sleeps_the_cat_on_its_own() {
        let mut m = launched();
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(0));
        let stale_after = DurationMs(1_800_000);
        let early = m.handle(Command::ExpireStale { stale_after }, at(1_799_999));
        assert_eq!(
            early,
            Handled {
                publish: false,
                hook: None
            }
        );
        assert_eq!(
            m.snapshot(false).sleep_cause,
            SleepCause::User,
            "untouched while nothing expires"
        );
        assert!(
            m.handle(Command::ExpireStale { stale_after }, at(1_800_000))
                .publish
        );
        let s = m.snapshot(false);
        assert!(!s.awake);
        assert_eq!(s.sleep_cause, SleepCause::Auto);
    }

    #[test]
    fn a_settings_change_is_nobody_s_hand() {
        let mut m = launched();
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(0));
        assert!(m.handle(Command::SettingsChanged, at(1)).publish);
        assert_eq!(m.snapshot(true).sleep_cause, SleepCause::Auto);
    }

    #[test]
    fn hooks_are_your_hand_even_when_ignored() {
        let mut m = launched();
        m.handle(Command::SettingsChanged, at(0));
        let handled = m.handle(Command::Hook(&hook("SubagentStop", "a")), at(1));
        assert_eq!(
            handled,
            Handled {
                publish: false,
                hook: Some(Outcome::Unchanged)
            }
        );
        assert_eq!(m.snapshot(false).sleep_cause, SleepCause::User);
        let cleared = m.handle(Command::Hook(&hook("UserPromptSubmit", "a")), at(2));
        assert_eq!(
            cleared.hook,
            Some(Outcome::Unchanged),
            "nothing was waiting"
        );
    }

    #[test]
    fn a_hook_reports_its_outcome_for_the_log() {
        let mut m = launched();
        let woke = m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(0));
        assert_eq!(
            woke,
            Handled {
                publish: true,
                hook: Some(Outcome::Attention {
                    project_name: "proj-a".into(),
                    reason: WakeReason::Permission
                })
            }
        );
        let cleared = m.handle(Command::Hook(&hook("UserPromptSubmit", "a")), at(10));
        assert_eq!(
            cleared,
            Handled {
                publish: true,
                hook: Some(Outcome::Changed)
            }
        );
        let s = m.snapshot(false);
        assert!(!s.awake);
        assert_eq!(s.sleep_cause, SleepCause::User);
    }

    #[test]
    fn the_yell_counter_moves_only_for_waits_that_may_wake_the_cat() {
        let mut m = launched();
        m.handle(Command::SetReady(false), at(0));
        m.handle(Command::Hook(&hook("Stop", "a")), at(1_000)); // Ready off: no yell
        assert_eq!(m.attention, None);
        m.handle(Command::Hook(&hook("PermissionRequest", "b")), at(2_000));
        assert_eq!(seq(&m, false), Some(1));
        m.handle(Command::Hook(&hook("PermissionRequest", "b")), at(2_500)); // duplicate
        assert_eq!(seq(&m, false), Some(1));
        m.handle(Command::Hook(&note("idle_prompt", "c")), at(3_000));
        assert_eq!(seq(&m, false), Some(2));
        assert_eq!(m.snapshot(false).attention.unwrap().project_name, "proj-c");
        m.handle(Command::WakeTest, at(4_000));
        assert_eq!(seq(&m, false), Some(3));
    }

    #[test]
    fn asleep_means_no_attention_even_if_one_is_remembered() {
        let mut m = launched();
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(0));
        m.handle(Command::DismissAll, at(1));
        assert!(m.attention.is_some());
        assert_eq!(m.snapshot(false).attention, None);
        // Woken again, the counter carries on.
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(5_000));
        assert_eq!(seq(&m, false), Some(2));
    }

    #[test]
    fn the_reason_is_the_newest_wait_and_falls_back_to_the_wake_test() {
        let mut m = launched();
        m.handle(Command::WakeTest, at(0));
        assert_eq!(m.snapshot(false).reason, Some(WakeReason::Done));
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(1_000));
        assert_eq!(m.snapshot(false).reason, Some(WakeReason::Permission));
        m.handle(Command::Hook(&hook("Stop", "b")), at(2_000));
        assert_eq!(m.snapshot(false).reason, Some(WakeReason::Done));
    }

    #[test]
    fn arming_ready_again_keeps_the_first_time_and_turning_it_off_is_your_hand() {
        let mut m = launched();
        m.handle(Command::Hook(&hook("Stop", "a")), at(1_000));
        assert!(m.snapshot(false).awake);
        m.handle(Command::SetReady(true), at(2_000)); // already armed at T0
        assert!(
            m.snapshot(false).awake,
            "the turn that ended after T0 still counts"
        );
        m.handle(Command::SetReady(false), at(3_000));
        let s = m.snapshot(false);
        assert!(!s.awake && !s.ready);
        assert_eq!(s.sleep_cause, SleepCause::User);
    }

    #[test]
    fn the_snapshot_keeps_the_names_the_overlay_reads() {
        assert_eq!(serde_json::to_value(SleepCause::User).unwrap(), "user");
        assert_eq!(serde_json::to_value(SleepCause::Auto).unwrap(), "auto");
        let mut m = launched();
        m.handle(Command::Hook(&hook("PermissionRequest", "a")), at(0));
        assert_eq!(
            serde_json::to_value(m.snapshot(false)).unwrap(),
            json!({
                "awake": true,
                "reason": "permission",
                "sessions": [{ "projectName": "proj-a", "reason": "permission" }],
                "attention": { "seq": 1, "projectName": "proj-a", "reason": "permission" },
                "ready": true,
                "sleepCause": "user"
            })
        );
        assert_eq!(
            serde_json::to_value(launched().snapshot(false)).unwrap(),
            json!({
                "awake": false, "reason": null, "sessions": [], "attention": null, "ready": true,
                "sleepCause": "auto"
            })
        );
    }
}
