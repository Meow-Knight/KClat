//! Which Claude Code sessions need you (spec §8.1–8.2). Pure logic: no Tauri, no I/O, no clock.
//!
//! Mapping, as verified against the official hooks docs (2026-10):
//! - `Stop`, `StopFailure` → needs attention (done). StopFailure is a turn that ended on an API error.
//! - `PermissionRequest` → needs attention (permission), the moment the prompt appears.
//! - `Notification` is a fallback only, ignored when the session is already pending:
//!   `permission_prompt` (prompt unanswered ~6s) → permission;
//!   `idle_prompt` (~60s after a reply), `agent_needs_input`, `elicitation_dialog`, `elicitation_url_dialog` → input.
//! - `UserPromptSubmit` → cleared. `PostToolUse` / `PostToolUseFailure` of the tool that asked permission → cleared.
//! - `PermissionDenied` (auto mode only) → clears a permission wait. `SessionEnd` → session removed.
//! - Anything else (including `SubagentStop`) is ignored.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Body of `POST /event` (spec §8.3). The adapter never sends tool input, commands or transcripts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    pub event_id: String,
    pub hook_event_name: String,
    #[serde(default)]
    pub notification_type: Option<String>,
    pub session_id: String,
    #[serde(default)]
    pub project_name: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    pub occurred_at: u64,
}

/// Hook names the server accepts; everything else is rejected before it reaches the store.
pub const ALLOWED_HOOKS: [&str; 10] = [
    "Stop",
    "StopFailure",
    "PermissionRequest",
    "Notification",
    "UserPromptSubmit",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionDenied",
    "SessionEnd",
    "SubagentStop",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WakeReason {
    Done,
    Permission,
    Input,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearCause {
    PromptSubmitted,
    ToolResumed,
    PermissionDenied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    AttentionNeeded {
        reason: WakeReason,
        /// From a Notification: only counts when the session is not already waiting.
        fallback: bool,
    },
    AttentionCleared(ClearCause),
    SessionEnded,
}

pub fn classify(ev: &HookEvent) -> Option<Signal> {
    use Signal::*;
    let needed = |reason, fallback| Some(AttentionNeeded { reason, fallback });
    match ev.hook_event_name.as_str() {
        "Stop" | "StopFailure" => needed(WakeReason::Done, false),
        "PermissionRequest" => needed(WakeReason::Permission, false),
        "Notification" => match ev.notification_type.as_deref()? {
            "permission_prompt" => needed(WakeReason::Permission, true),
            "idle_prompt"
            | "agent_needs_input"
            | "elicitation_dialog"
            | "elicitation_url_dialog" => needed(WakeReason::Input, true),
            _ => None,
        },
        "UserPromptSubmit" => Some(AttentionCleared(ClearCause::PromptSubmitted)),
        "PostToolUse" | "PostToolUseFailure" => Some(AttentionCleared(ClearCause::ToolResumed)),
        "PermissionDenied" => Some(AttentionCleared(ClearCause::PermissionDenied)),
        "SessionEnd" => Some(SessionEnded),
        _ => None,
    }
}

/// Same session + same reason within this window is a duplicate: no re-wake, no yell-counter reset.
pub const DEDUPE_MS: u64 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub pending: bool,
    pub reason: WakeReason,
    /// When it became pending (unix ms).
    pub since: u64,
    pub project_name: String,
    pub tool_name: Option<String>,
    pub dismissed: bool,
    last_attention: Option<(WakeReason, u64)>,
}

/// What an applied event did, for the caller to publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing changed (ignored, duplicate, or no-op).
    Unchanged,
    /// The session now needs you: wake the cat, or yell again if it is already awake.
    Attention {
        project_name: String,
        reason: WakeReason,
    },
    /// Something changed that may put the cat to sleep (cleared, ended).
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub project_name: String,
    pub reason: WakeReason,
}

#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    pub fn apply(&mut self, ev: &HookEvent, now: u64) -> Outcome {
        let Some(signal) = classify(ev) else {
            return Outcome::Unchanged;
        };
        let project = ev.project_name.clone().filter(|p| !p.is_empty());
        match signal {
            Signal::AttentionNeeded { reason, fallback } => {
                let s = self
                    .sessions
                    .entry(ev.session_id.clone())
                    .or_insert_with(|| Session {
                        pending: false,
                        reason,
                        since: now,
                        project_name: String::new(),
                        tool_name: None,
                        dismissed: false,
                        last_attention: None,
                    });
                if fallback && s.pending {
                    return Outcome::Unchanged;
                }
                if let Some((r, at)) = s.last_attention {
                    if r == reason && now.saturating_sub(at) < DEDUPE_MS {
                        return Outcome::Unchanged;
                    }
                }
                if !s.pending {
                    s.since = now;
                }
                s.pending = true;
                s.reason = reason;
                s.dismissed = false;
                s.last_attention = Some((reason, now));
                s.tool_name = if reason == WakeReason::Permission {
                    ev.tool_name.clone()
                } else {
                    None
                };
                if let Some(p) = project {
                    s.project_name = p;
                }
                Outcome::Attention {
                    project_name: s.project_name.clone(),
                    reason,
                }
            }
            Signal::AttentionCleared(cause) => {
                let Some(s) = self.sessions.get_mut(&ev.session_id) else {
                    return Outcome::Unchanged;
                };
                if !s.pending {
                    return Outcome::Unchanged;
                }
                let clears = match cause {
                    ClearCause::PromptSubmitted => true,
                    // Only the tool that was waiting for approval; no tool name on record (Notification
                    // fallback) means any tool running again shows you answered.
                    ClearCause::ToolResumed => {
                        s.reason == WakeReason::Permission
                            && match (&s.tool_name, &ev.tool_name) {
                                (Some(waiting), Some(ran)) => waiting == ran,
                                (None, _) => true,
                                (Some(_), None) => false,
                            }
                    }
                    ClearCause::PermissionDenied => s.reason == WakeReason::Permission,
                };
                if !clears {
                    return Outcome::Unchanged;
                }
                s.pending = false;
                s.dismissed = false;
                s.tool_name = None;
                Outcome::Changed
            }
            Signal::SessionEnded => match self.sessions.remove(&ev.session_id) {
                Some(_) => Outcome::Changed,
                None => Outcome::Unchanged,
            },
        }
    }

    /// Marks every waiting session as seen. Returns whether anything changed. Sends nothing to Claude Code.
    pub fn dismiss_all(&mut self) -> bool {
        let mut changed = false;
        for s in self
            .sessions
            .values_mut()
            .filter(|s| s.pending && !s.dismissed)
        {
            s.dismissed = true;
            changed = true;
        }
        changed
    }

    /// Clears sessions that have been waiting longer than `stale_after_ms`. Returns whether anything changed.
    pub fn expire(&mut self, now: u64, stale_after_ms: u64) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.pending && now.saturating_sub(s.since) >= stale_after_ms {
                s.pending = false;
                s.dismissed = false;
                s.tool_name = None;
                changed = true;
            }
        }
        changed
    }

    /// Sessions currently calling for you (pending and not dismissed), oldest first.
    pub fn waiting(&self) -> Vec<SessionView> {
        let mut v: Vec<(&Session, SessionView)> = self
            .sessions
            .values()
            .filter(|s| s.pending && !s.dismissed)
            .map(|s| {
                let view = SessionView {
                    project_name: s.project_name.clone(),
                    reason: s.reason,
                };
                (s, view)
            })
            .collect();
        v.sort_by_key(|(s, _)| s.since);
        v.into_iter().map(|(_, view)| view).collect()
    }

    pub fn awake(&self, paused: bool) -> bool {
        !paused && self.sessions.values().any(|s| s.pending && !s.dismissed)
    }

    #[cfg(test)]
    fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str, session: &str) -> HookEvent {
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
            ..ev("Notification", session)
        }
    }
    fn tool(name: &str, session: &str, tool: &str) -> HookEvent {
        HookEvent {
            tool_name: Some(tool.into()),
            ..ev(name, session)
        }
    }
    fn attention(project: &str, reason: WakeReason) -> Outcome {
        Outcome::Attention {
            project_name: project.into(),
            reason,
        }
    }

    #[test]
    fn mapping_matches_the_spec_and_the_docs() {
        use WakeReason::*;
        let need = |r, f| {
            Some(Signal::AttentionNeeded {
                reason: r,
                fallback: f,
            })
        };
        assert_eq!(classify(&ev("Stop", "a")), need(Done, false));
        assert_eq!(classify(&ev("StopFailure", "a")), need(Done, false));
        assert_eq!(
            classify(&ev("PermissionRequest", "a")),
            need(Permission, false)
        );
        assert_eq!(
            classify(&note("permission_prompt", "a")),
            need(Permission, true)
        );
        for kind in [
            "idle_prompt",
            "agent_needs_input",
            "elicitation_dialog",
            "elicitation_url_dialog",
        ] {
            assert_eq!(classify(&note(kind, "a")), need(Input, true), "{kind}");
        }
        for kind in [
            "auth_success",
            "agent_completed",
            "elicitation_complete",
            "weird",
        ] {
            assert_eq!(classify(&note(kind, "a")), None, "{kind}");
        }
        assert_eq!(classify(&ev("Notification", "a")), None); // no notification_type
        assert_eq!(
            classify(&ev("UserPromptSubmit", "a")),
            Some(Signal::AttentionCleared(ClearCause::PromptSubmitted))
        );
        assert_eq!(
            classify(&ev("PostToolUseFailure", "a")),
            Some(Signal::AttentionCleared(ClearCause::ToolResumed))
        );
        assert_eq!(
            classify(&ev("PermissionDenied", "a")),
            Some(Signal::AttentionCleared(ClearCause::PermissionDenied))
        );
        assert_eq!(classify(&ev("SessionEnd", "a")), Some(Signal::SessionEnded));
        assert_eq!(classify(&ev("SubagentStop", "a")), None);
        assert_eq!(classify(&ev("PreToolUse", "a")), None);
    }

    #[test]
    fn stop_wakes_and_prompt_submit_sleeps() {
        let mut st = SessionStore::default();
        assert_eq!(
            st.apply(&ev("Stop", "a"), 1_000),
            attention("proj-a", WakeReason::Done)
        );
        assert!(st.awake(false));
        assert_eq!(
            st.waiting(),
            vec![SessionView {
                project_name: "proj-a".into(),
                reason: WakeReason::Done
            }]
        );
        assert_eq!(
            st.apply(&ev("UserPromptSubmit", "a"), 2_000),
            Outcome::Changed
        );
        assert!(!st.awake(false));
        assert!(st.waiting().is_empty());
    }

    #[test]
    fn duplicates_within_2s_are_ignored() {
        let mut st = SessionStore::default();
        assert!(matches!(
            st.apply(&ev("Stop", "a"), 10_000),
            Outcome::Attention { .. }
        ));
        assert_eq!(st.apply(&ev("Stop", "a"), 11_999), Outcome::Unchanged);
        // Past the window it counts again (e.g. a second turn ended).
        assert!(matches!(
            st.apply(&ev("Stop", "a"), 12_000),
            Outcome::Attention { .. }
        ));
        // A different reason inside the window is not a duplicate.
        assert!(matches!(
            st.apply(&ev("PermissionRequest", "a"), 12_500),
            Outcome::Attention { .. }
        ));
    }

    #[test]
    fn notification_fallbacks_only_count_when_not_already_waiting() {
        let mut st = SessionStore::default();
        // PermissionRequest first, permission_prompt ~6s later: no second wake.
        st.apply(&tool("PermissionRequest", "a", "Bash"), 0);
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), 6_000),
            Outcome::Unchanged
        );
        // ...even after you dismissed it.
        st.dismiss_all();
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), 7_000),
            Outcome::Unchanged
        );
        assert!(!st.awake(false));
        // idle_prompt 60s after a Stop is ignored too.
        st.apply(&ev("Stop", "b"), 0);
        assert_eq!(
            st.apply(&note("idle_prompt", "b"), 60_000),
            Outcome::Unchanged
        );
        // But a fallback alone (no PermissionRequest seen) does wake.
        assert_eq!(
            st.apply(&note("permission_prompt", "c"), 0),
            attention("proj-c", WakeReason::Permission)
        );
    }

    #[test]
    fn events_of_one_session_never_touch_another() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        st.apply(&tool("PermissionRequest", "b", "Edit"), 0);
        st.apply(&ev("UserPromptSubmit", "a"), 1_000);
        st.apply(&ev("SessionEnd", "c"), 1_000);
        assert!(!st.get("a").unwrap().pending);
        assert!(st.get("b").unwrap().pending);
        assert_eq!(st.waiting().len(), 1);
        assert!(st.awake(false));
    }

    #[test]
    fn permission_cleared_by_the_same_tool_running() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), 0);
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Edit"), 100),
            Outcome::Unchanged
        );
        assert!(st.awake(false));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), 200),
            Outcome::Changed
        );
        assert!(!st.awake(false));
        // PostToolUseFailure means the tool ran too (approved, then failed).
        st.apply(&tool("PermissionRequest", "a", "Write"), 5_000);
        assert_eq!(
            st.apply(&tool("PostToolUseFailure", "a", "Write"), 5_100),
            Outcome::Changed
        );
    }

    #[test]
    fn tool_events_do_not_clear_a_done_wait() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), 10),
            Outcome::Unchanged
        );
        assert_eq!(
            st.apply(&ev("PermissionDenied", "a"), 10),
            Outcome::Unchanged
        );
        assert!(st.awake(false));
    }

    #[test]
    fn fallback_permission_without_tool_is_cleared_by_any_tool() {
        let mut st = SessionStore::default();
        st.apply(&note("permission_prompt", "a"), 0);
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Read"), 10),
            Outcome::Changed
        );
    }

    #[test]
    fn permission_denied_clears_a_permission_wait() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), 0);
        assert_eq!(
            st.apply(&tool("PermissionDenied", "a", "Bash"), 10),
            Outcome::Changed
        );
        assert!(!st.awake(false));
    }

    #[test]
    fn clears_and_ends_for_unknown_sessions_are_no_ops() {
        let mut st = SessionStore::default();
        assert_eq!(
            st.apply(&ev("UserPromptSubmit", "x"), 0),
            Outcome::Unchanged
        );
        assert_eq!(st.apply(&ev("SessionEnd", "x"), 0), Outcome::Unchanged);
        assert!(st.get("x").is_none());
    }

    #[test]
    fn session_end_removes_the_session() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        assert_eq!(st.apply(&ev("SessionEnd", "a"), 10), Outcome::Changed);
        assert!(st.get("a").is_none());
        assert!(!st.awake(false));
    }

    #[test]
    fn dismiss_quiets_pending_sessions_until_they_need_you_again() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        st.apply(&ev("Stop", "b"), 0);
        assert!(st.dismiss_all());
        assert!(!st.dismiss_all()); // nothing left to dismiss
        assert!(!st.awake(false));
        assert!(st.get("a").unwrap().pending); // still pending, just seen
                                               // A new attention_needed for a resets dismissed for a only.
        assert!(matches!(
            st.apply(&tool("PermissionRequest", "a", "Bash"), 5_000),
            Outcome::Attention { .. }
        ));
        assert!(st.awake(false));
        assert!(!st.get("a").unwrap().dismissed);
        assert!(st.get("b").unwrap().dismissed);
    }

    #[test]
    fn pause_keeps_the_cat_asleep_without_losing_state() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        assert!(!st.awake(true));
        assert!(st.awake(false));
    }

    #[test]
    fn stale_pending_sessions_expire() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), 0);
        st.apply(&ev("Stop", "b"), 1_000_000);
        assert!(!st.expire(1_799_999, 1_800_000));
        assert!(st.expire(1_800_000, 1_800_000));
        assert!(!st.get("a").unwrap().pending);
        assert!(st.get("b").unwrap().pending);
    }

    #[test]
    fn since_is_kept_while_already_pending_and_waiting_is_oldest_first() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "b"), 5_000);
        st.apply(&ev("Stop", "a"), 1_000);
        st.apply(&tool("PermissionRequest", "b", "Bash"), 9_000); // b changes reason, keeps since
        assert_eq!(st.get("b").unwrap().since, 5_000);
        let names: Vec<_> = st.waiting().into_iter().map(|v| v.project_name).collect();
        assert_eq!(names, vec!["proj-a", "proj-b"]);
    }
}
