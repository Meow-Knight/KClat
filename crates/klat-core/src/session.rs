//! Which Claude Code and Codex sessions need you (spec §8.1–8.2). Pure logic: no Tauri, no I/O, no clock.
//!
//! Mapping, as verified against the official hooks docs of both (2026-10). Codex sends the same names with the
//! same meaning, but only `Stop`, `PermissionRequest`, `UserPromptSubmit`, `PostToolUse`, `SessionEnd` and
//! `Interrupt` (so no "waiting for input" fallback):
//! - `Stop`, `StopFailure` → needs attention (done). StopFailure is a turn that ended on an API error.
//! - `PermissionRequest` → needs attention (permission), the moment the prompt appears.
//! - `Notification` is a fallback only, ignored when the session is already pending:
//!   `permission_prompt` (prompt unanswered ~6s) → permission;
//!   `idle_prompt` (~60s after a reply), `agent_needs_input`, `elicitation_dialog`, `elicitation_url_dialog` → input.
//! - `UserPromptSubmit` → cleared. `PostToolUse` / `PostToolUseFailure` of the tool that asked permission → cleared.
//! - `PostToolBatch` (Claude Code, not from a subagent) → clears a permission wait: every call of the batch was
//!   answered, including one you refused with a message for Claude.
//! - `PermissionDenied` (auto mode only) → clears a permission wait. `SessionEnd` → session removed.
//! - `Interrupt` (Codex: you stopped the turn) → cleared.
//! - Anything else (including `SubagentStop`, and any event from an agent this version does not know) is ignored.
//!
//! No hook runs when you answer a permission prompt: Yes shows only when the tool finishes, and No or Esc ends the
//! turn without any hook. But Claude Code sends `Notification(permission_prompt)` only for a prompt left
//! unanswered for ~6s, so a Claude Code prompt with no such notification by [`PROMPT_ANSWERED_AFTER`] was
//! answered ([`SessionStore::clear_answered`]). A notification that arrives later still wakes the cat again.

use crate::time::{DurationMs, UnixMs};
use klat_protocol::{Agent, HookEvent, HookName};
use serde::Serialize;
use std::collections::HashMap;

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
    BatchResolved,
    PermissionDenied,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    AttentionNeeded {
        reason: WakeReason,
        /// From a Notification: only counts when the session is not already waiting.
        fallback: bool,
        /// A prompt the agent reports within ~6s if it is still unanswered (Claude Code's
        /// `Notification(permission_prompt)`), so silence means you answered it.
        silence_means_answered: bool,
    },
    AttentionCleared(ClearCause),
    SessionEnded,
}

/// What a hook means for the cat. Every `HookName` is matched by name, so a hook added to klat-protocol does not
/// compile until it is decided here.
pub fn classify(ev: &HookEvent) -> Option<Signal> {
    use Signal::*;
    let needed = |reason, fallback| {
        Some(AttentionNeeded {
            reason,
            fallback,
            silence_means_answered: false,
        })
    };
    let agent = ev.agent()?;
    match ev.hook()? {
        HookName::Stop | HookName::StopFailure => needed(WakeReason::Done, false),
        HookName::PermissionRequest => Some(AttentionNeeded {
            reason: WakeReason::Permission,
            fallback: false,
            // Codex has no Notification hook.
            silence_means_answered: agent == Agent::ClaudeCode,
        }),
        HookName::Notification => match ev.notification_type.as_deref()? {
            "permission_prompt" => needed(WakeReason::Permission, true),
            "idle_prompt"
            | "agent_needs_input"
            | "elicitation_dialog"
            | "elicitation_url_dialog" => needed(WakeReason::Input, true),
            _ => None,
        },
        HookName::UserPromptSubmit => Some(AttentionCleared(ClearCause::PromptSubmitted)),
        HookName::PostToolUse | HookName::PostToolUseFailure => {
            Some(AttentionCleared(ClearCause::ToolResumed))
        }
        // A subagent's batch says nothing about the prompt the main agent (or another subagent) is showing.
        HookName::PostToolBatch if ev.subagent => None,
        HookName::PostToolBatch => Some(AttentionCleared(ClearCause::BatchResolved)),
        HookName::PermissionDenied => Some(AttentionCleared(ClearCause::PermissionDenied)),
        HookName::SessionEnd => Some(SessionEnded),
        HookName::Interrupt => Some(AttentionCleared(ClearCause::Interrupted)),
        HookName::SubagentStop => None,
    }
}

/// Same session + same reason within this window is a duplicate: no re-wake, no yell-counter reset.
pub const DEDUPE_WINDOW: DurationMs = DurationMs(2_000);

/// A Claude Code permission prompt with no `Notification(permission_prompt)` this long after it appeared was
/// answered. Claude Code sends that notification once the prompt has waited ~6s; the rest is room for the hook to
/// start and arrive.
pub const PROMPT_ANSWERED_AFTER: DurationMs = DurationMs(8_000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub pending: bool,
    pub reason: WakeReason,
    /// When it became pending.
    pub since: UnixMs,
    pub project_name: String,
    pub tool_name: Option<String>,
    pub dismissed: bool,
    /// When a Claude Code permission prompt appeared that it has not yet reported as left unanswered: once
    /// [`PROMPT_ANSWERED_AFTER`] has passed, you answered it.
    prompt_shown: Option<UnixMs>,
    last_attention: Option<(WakeReason, UnixMs)>,
    /// The last event for it, or when its wait expired. Quiet sessions are forgotten by `expire`.
    last_seen: UnixMs,
}

/// Sessions the store keeps at most. Room for a new one is made from sessions that are not waiting; when all of
/// them are waiting, a new session is ignored until one of them clears.
pub const MAX_SESSIONS: usize = 256;

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

#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub project_name: String,
    pub reason: WakeReason,
}

/// Which waiting sessions may wake the cat.
///
/// - Paused: none.
/// - Permission and input waits (Claude is blocked on you): always.
/// - "Claude is done" waits: only when Ready was armed before the session became pending. Without Ready the
///   cat keeps sleeping through finished turns, so it does not distract someone already watching Claude.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate {
    pub paused: bool,
    /// When Ready was armed, if it is.
    pub ready_since: Option<UnixMs>,
}

impl Gate {
    fn lets_through(&self, s: &Session) -> bool {
        s.pending
            && !s.dismissed
            && (s.reason != WakeReason::Done
                || self.ready_since.is_some_and(|armed| s.since >= armed))
    }
}

impl Session {
    /// No longer waiting.
    fn clear(&mut self) {
        self.pending = false;
        self.dismissed = false;
        self.tool_name = None;
        self.prompt_shown = None;
    }
}

#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: HashMap<String, Session>,
}

impl SessionStore {
    pub fn apply(&mut self, ev: &HookEvent, now: UnixMs) -> Outcome {
        let Some(signal) = classify(ev) else {
            return Outcome::Unchanged;
        };
        let project = ev.project_name.clone().filter(|p| !p.is_empty());
        match signal {
            Signal::AttentionNeeded {
                reason,
                fallback,
                silence_means_answered,
            } => {
                if !self.sessions.contains_key(&ev.session_id)
                    && self.sessions.len() >= MAX_SESSIONS
                    && !self.make_room()
                {
                    return Outcome::Unchanged;
                }
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
                        prompt_shown: None,
                        last_attention: None,
                        last_seen: now,
                    });
                s.last_seen = now;
                if fallback && s.pending {
                    if reason == WakeReason::Permission && s.reason == WakeReason::Permission {
                        // The prompt is still on screen: only an answer clears it now.
                        s.prompt_shown = None;
                    }
                    return Outcome::Unchanged;
                }
                if let Some((r, at)) = s.last_attention {
                    if r == reason && now.since(at) < DEDUPE_WINDOW {
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
                s.prompt_shown = silence_means_answered.then_some(now);
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
                s.last_seen = now;
                if !s.pending {
                    return Outcome::Unchanged;
                }
                let clears = match cause {
                    ClearCause::PromptSubmitted | ClearCause::Interrupted => true,
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
                    ClearCause::BatchResolved | ClearCause::PermissionDenied => {
                        s.reason == WakeReason::Permission
                    }
                };
                if !clears {
                    return Outcome::Unchanged;
                }
                s.clear();
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

    /// Drops the quietest session that is not waiting. Returns false when every session is waiting.
    fn make_room(&mut self) -> bool {
        let quiet = self
            .sessions
            .iter()
            .filter(|(_, s)| !s.pending)
            .min_by_key(|(_, s)| s.last_seen)
            .map(|(id, _)| id.clone());
        quiet.and_then(|id| self.sessions.remove(&id)).is_some()
    }

    /// Clears sessions that have been waiting longer than `stale_after`, and forgets sessions that have not
    /// been waiting nor heard from for that long (SessionEnd does not always come). Returns whether anything
    /// visible changed.
    pub fn expire(&mut self, now: UnixMs, stale_after: DurationMs) -> bool {
        self.sessions
            .retain(|_, s| s.pending || now.since(s.last_seen) < stale_after);
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.pending && now.since(s.since) >= stale_after {
                s.last_seen = now;
                s.clear();
                changed = true;
            }
        }
        changed
    }

    /// Clears permission waits whose prompt Claude Code did not report as left unanswered within
    /// [`PROMPT_ANSWERED_AFTER`]: you answered it (see the module docs). Returns whether anything changed.
    pub fn clear_answered(&mut self, now: UnixMs) -> bool {
        let mut changed = false;
        for s in self.sessions.values_mut() {
            if s.pending
                && s.prompt_shown
                    .is_some_and(|shown| now.since(shown) >= PROMPT_ANSWERED_AFTER)
            {
                s.last_seen = now;
                s.clear();
                changed = true;
            }
        }
        changed
    }

    /// Sessions currently calling for you, oldest first (see [`Gate`]).
    pub fn waiting(&self, gate: Gate) -> Vec<SessionView> {
        if gate.paused {
            return Vec::new();
        }
        let mut v: Vec<(&Session, SessionView)> = self
            .sessions
            .values()
            .filter(|s| gate.lets_through(s))
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

    /// Whether this session's current wait may wake the cat under `gate`.
    pub fn calls_for_you(&self, session_id: &str, gate: Gate) -> bool {
        !gate.paused
            && self
                .sessions
                .get(session_id)
                .is_some_and(|s| gate.lets_through(s))
    }

    pub fn awake(&self, gate: Gate) -> bool {
        !gate.paused && self.sessions.values().any(|s| gate.lets_through(s))
    }

    #[cfg(test)]
    fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn t(ms: u64) -> UnixMs {
        UnixMs(ms)
    }

    fn ev(name: &str, session: &str) -> HookEvent {
        HookEvent {
            event_id: "e".into(),
            hook_event_name: name.into(),
            notification_type: None,
            session_id: session.into(),
            project_name: Some(format!("proj-{session}")),
            tool_name: None,
            occurred_at: 0,
            agent: None,
            subagent: false,
        }
    }
    fn note(kind: &str, session: &str) -> HookEvent {
        HookEvent {
            notification_type: Some(kind.into()),
            ..ev("Notification", session)
        }
    }
    fn codex(name: &str, session: &str) -> HookEvent {
        HookEvent {
            agent: Some("codex".into()),
            ..ev(name, session)
        }
    }
    fn tool(name: &str, session: &str, tool: &str) -> HookEvent {
        HookEvent {
            tool_name: Some(tool.into()),
            ..ev(name, session)
        }
    }
    /// Ready armed from the start: "done" waits wake the cat, as before Ready existed.
    const READY: Gate = Gate {
        paused: false,
        ready_since: Some(t(0)),
    };
    const PAUSED: Gate = Gate {
        paused: true,
        ready_since: Some(t(0)),
    };

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
                silence_means_answered: false,
            })
        };
        assert_eq!(classify(&ev("Stop", "a")), need(Done, false));
        assert_eq!(classify(&ev("StopFailure", "a")), need(Done, false));
        assert_eq!(
            classify(&ev("PermissionRequest", "a")),
            Some(Signal::AttentionNeeded {
                reason: Permission,
                fallback: false,
                silence_means_answered: true,
            })
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
            classify(&ev("PostToolBatch", "a")),
            Some(Signal::AttentionCleared(ClearCause::BatchResolved))
        );
        let in_subagent = HookEvent {
            subagent: true,
            ..ev("PostToolBatch", "a")
        };
        assert_eq!(classify(&in_subagent), None);
        assert_eq!(
            classify(&ev("PermissionDenied", "a")),
            Some(Signal::AttentionCleared(ClearCause::PermissionDenied))
        );
        assert_eq!(classify(&ev("SessionEnd", "a")), Some(Signal::SessionEnded));
        assert_eq!(classify(&ev("SubagentStop", "a")), None);
        assert_eq!(classify(&ev("PreToolUse", "a")), None);
        assert_eq!(
            classify(&ev("Interrupt", "a")),
            Some(Signal::AttentionCleared(ClearCause::Interrupted))
        );
    }

    #[test]
    fn codex_events_mean_what_claude_code_events_mean() {
        for name in ["Stop", "UserPromptSubmit", "PostToolUse", "SessionEnd"] {
            assert_eq!(
                classify(&codex(name, "a")),
                classify(&ev(name, "a")),
                "{name}"
            );
        }
        let unknown = HookEvent {
            agent: Some("cursor".into()),
            ..ev("Stop", "a")
        };
        assert_eq!(classify(&unknown), None);
        // Except that Codex never says a prompt is still unanswered: it has no Notification hook.
        assert_eq!(
            classify(&codex("PermissionRequest", "a")),
            Some(Signal::AttentionNeeded {
                reason: WakeReason::Permission,
                fallback: false,
                silence_means_answered: false,
            })
        );
    }

    #[test]
    fn a_prompt_not_reported_unanswered_counts_as_answered() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(1_000));
        assert!(!st.clear_answered(t(8_999)));
        assert!(st.awake(READY));
        // You pressed Yes on a long build (or No): no hook says so, and no permission_prompt came.
        assert!(st.clear_answered(t(9_000)));
        assert!(!st.awake(READY));
        assert!(!st.clear_answered(t(10_000)), "only once");
        // The build finishing later changes nothing.
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), t(120_000)),
            Outcome::Unchanged
        );
    }

    #[test]
    fn a_prompt_reported_unanswered_waits_for_a_real_answer() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), t(6_000)),
            Outcome::Unchanged
        );
        assert!(!st.clear_answered(t(600_000)));
        assert!(st.awake(READY));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), t(601_000)),
            Outcome::Changed
        );
        // The next prompt gets its own window.
        st.apply(&tool("PermissionRequest", "a", "Edit"), t(700_000));
        assert!(st.clear_answered(t(708_000)));
    }

    #[test]
    fn a_late_unanswered_report_wakes_the_cat_again() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        assert!(st.clear_answered(t(8_000)));
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), t(8_500)),
            attention("proj-a", WakeReason::Permission)
        );
        assert!(!st.clear_answered(t(30_000)), "a fallback has no window");
        assert!(st.awake(READY));
    }

    #[test]
    fn only_claude_code_permission_prompts_count_as_answered() {
        let mut st = SessionStore::default();
        st.apply(&codex("PermissionRequest", "cx"), t(0));
        st.apply(&ev("Stop", "done"), t(0));
        st.apply(&note("agent_needs_input", "input"), t(0));
        st.apply(&note("permission_prompt", "fallback"), t(0));
        // A permission prompt that became a finished turn is not a prompt any more.
        st.apply(&tool("PermissionRequest", "then-done", "Bash"), t(0));
        st.apply(&ev("Stop", "then-done"), t(5_000));
        assert!(!st.clear_answered(t(600_000)));
        assert_eq!(st.waiting(READY).len(), 5);
    }

    #[test]
    fn a_resolved_batch_clears_a_permission_wait_whatever_the_tool() {
        let mut st = SessionStore::default();
        // You refused Bash with a message: Claude goes on with other tools, then the batch resolves.
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        st.apply(&note("permission_prompt", "a"), t(6_000));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Read"), t(30_000)),
            Outcome::Unchanged
        );
        assert_eq!(
            st.apply(&ev("PostToolBatch", "a"), t(30_100)),
            Outcome::Changed
        );
        assert!(!st.awake(READY));
        // It does not clear a finished turn, and a subagent's batch clears nothing.
        st.apply(&ev("Stop", "b"), t(0));
        assert_eq!(
            st.apply(&ev("PostToolBatch", "b"), t(10)),
            Outcome::Unchanged
        );
        st.apply(&tool("PermissionRequest", "c", "Bash"), t(0));
        let sub = HookEvent {
            subagent: true,
            ..ev("PostToolBatch", "c")
        };
        assert_eq!(st.apply(&sub, t(10)), Outcome::Unchanged);
        assert!(st.get("c").unwrap().pending);
    }

    #[test]
    fn interrupting_a_codex_turn_clears_its_wait() {
        let mut st = SessionStore::default();
        st.apply(&codex("PermissionRequest", "a"), t(1_000));
        assert!(st.awake(READY));
        assert_eq!(
            st.apply(&codex("Interrupt", "a"), t(2_000)),
            Outcome::Changed
        );
        assert!(!st.awake(READY));
    }

    #[test]
    fn stop_wakes_and_prompt_submit_sleeps() {
        let mut st = SessionStore::default();
        assert_eq!(
            st.apply(&ev("Stop", "a"), t(1_000)),
            attention("proj-a", WakeReason::Done)
        );
        assert!(st.awake(READY));
        assert_eq!(
            st.waiting(READY),
            vec![SessionView {
                project_name: "proj-a".into(),
                reason: WakeReason::Done
            }]
        );
        assert_eq!(
            st.apply(&ev("UserPromptSubmit", "a"), t(2_000)),
            Outcome::Changed
        );
        assert!(!st.awake(READY));
        assert!(st.waiting(READY).is_empty());
    }

    #[test]
    fn duplicates_within_2s_are_ignored() {
        let mut st = SessionStore::default();
        assert!(matches!(
            st.apply(&ev("Stop", "a"), t(10_000)),
            Outcome::Attention { .. }
        ));
        assert_eq!(st.apply(&ev("Stop", "a"), t(11_999)), Outcome::Unchanged);
        // Past the window it counts again (e.g. a second turn ended).
        assert!(matches!(
            st.apply(&ev("Stop", "a"), t(12_000)),
            Outcome::Attention { .. }
        ));
        // A different reason inside the window is not a duplicate.
        assert!(matches!(
            st.apply(&ev("PermissionRequest", "a"), t(12_500)),
            Outcome::Attention { .. }
        ));
    }

    #[test]
    fn notification_fallbacks_only_count_when_not_already_waiting() {
        let mut st = SessionStore::default();
        // PermissionRequest first, permission_prompt ~6s later: no second wake.
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), t(6_000)),
            Outcome::Unchanged
        );
        // ...even after you dismissed it.
        st.dismiss_all();
        assert_eq!(
            st.apply(&note("permission_prompt", "a"), t(7_000)),
            Outcome::Unchanged
        );
        assert!(!st.awake(READY));
        // idle_prompt 60s after a Stop is ignored too.
        st.apply(&ev("Stop", "b"), t(0));
        assert_eq!(
            st.apply(&note("idle_prompt", "b"), t(60_000)),
            Outcome::Unchanged
        );
        // But a fallback alone (no PermissionRequest seen) does wake.
        assert_eq!(
            st.apply(&note("permission_prompt", "c"), t(0)),
            attention("proj-c", WakeReason::Permission)
        );
    }

    #[test]
    fn events_of_one_session_never_touch_another() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        st.apply(&tool("PermissionRequest", "b", "Edit"), t(0));
        st.apply(&ev("UserPromptSubmit", "a"), t(1_000));
        st.apply(&ev("SessionEnd", "c"), t(1_000));
        assert!(!st.get("a").unwrap().pending);
        assert!(st.get("b").unwrap().pending);
        assert_eq!(st.waiting(READY).len(), 1);
        assert!(st.awake(READY));
    }

    #[test]
    fn permission_cleared_by_the_same_tool_running() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Edit"), t(100)),
            Outcome::Unchanged
        );
        assert!(st.awake(READY));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), t(200)),
            Outcome::Changed
        );
        assert!(!st.awake(READY));
        // PostToolUseFailure means the tool ran too (approved, then failed).
        st.apply(&tool("PermissionRequest", "a", "Write"), t(5_000));
        assert_eq!(
            st.apply(&tool("PostToolUseFailure", "a", "Write"), t(5_100)),
            Outcome::Changed
        );
    }

    #[test]
    fn tool_events_do_not_clear_a_done_wait() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Bash"), t(10)),
            Outcome::Unchanged
        );
        assert_eq!(
            st.apply(&ev("PermissionDenied", "a"), t(10)),
            Outcome::Unchanged
        );
        assert!(st.awake(READY));
    }

    #[test]
    fn fallback_permission_without_tool_is_cleared_by_any_tool() {
        let mut st = SessionStore::default();
        st.apply(&note("permission_prompt", "a"), t(0));
        assert_eq!(
            st.apply(&tool("PostToolUse", "a", "Read"), t(10)),
            Outcome::Changed
        );
    }

    #[test]
    fn permission_denied_clears_a_permission_wait() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        assert_eq!(
            st.apply(&tool("PermissionDenied", "a", "Bash"), t(10)),
            Outcome::Changed
        );
        assert!(!st.awake(READY));
    }

    #[test]
    fn clears_and_ends_for_unknown_sessions_are_no_ops() {
        let mut st = SessionStore::default();
        assert_eq!(
            st.apply(&ev("UserPromptSubmit", "x"), t(0)),
            Outcome::Unchanged
        );
        assert_eq!(st.apply(&ev("SessionEnd", "x"), t(0)), Outcome::Unchanged);
        assert!(st.get("x").is_none());
    }

    #[test]
    fn session_end_removes_the_session() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        assert_eq!(st.apply(&ev("SessionEnd", "a"), t(10)), Outcome::Changed);
        assert!(st.get("a").is_none());
        assert!(!st.awake(READY));
    }

    #[test]
    fn dismiss_quiets_pending_sessions_until_they_need_you_again() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        st.apply(&ev("Stop", "b"), t(0));
        assert!(st.dismiss_all());
        assert!(!st.dismiss_all()); // nothing left to dismiss
        assert!(!st.awake(READY));
        assert!(st.get("a").unwrap().pending); // still pending, just seen
                                               // A new attention_needed for a resets dismissed for a only.
        assert!(matches!(
            st.apply(&tool("PermissionRequest", "a", "Bash"), t(5_000)),
            Outcome::Attention { .. }
        ));
        assert!(st.awake(READY));
        assert!(!st.get("a").unwrap().dismissed);
        assert!(st.get("b").unwrap().dismissed);
    }

    #[test]
    fn pause_keeps_the_cat_asleep_without_losing_state() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        assert!(!st.awake(PAUSED));
        assert!(st.awake(READY));
    }

    #[test]
    fn without_ready_done_waits_do_not_wake_but_permission_and_input_do() {
        let normal = Gate::default();
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(1_000));
        assert!(!st.awake(normal));
        assert!(st.waiting(normal).is_empty());
        st.apply(&tool("PermissionRequest", "b", "Bash"), t(2_000));
        assert!(st.awake(normal));
        assert_eq!(
            st.waiting(normal),
            vec![SessionView {
                project_name: "proj-b".into(),
                reason: WakeReason::Permission
            }]
        );
        st.apply(&note("agent_needs_input", "c"), t(3_000));
        assert_eq!(st.waiting(normal).len(), 2);
    }

    #[test]
    fn ready_counts_only_done_waits_that_start_after_arming() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "old"), t(1_000)); // finished before you armed Ready
        let armed = Gate {
            paused: false,
            ready_since: Some(t(5_000)),
        };
        assert!(!st.awake(armed));
        st.apply(&ev("Stop", "new"), t(5_000));
        assert!(st.awake(armed));
        let names: Vec<_> = st
            .waiting(armed)
            .into_iter()
            .map(|v| v.project_name)
            .collect();
        assert_eq!(names, vec!["proj-new"]);
    }

    #[test]
    fn ready_stays_armed_across_turns() {
        let mut st = SessionStore::default();
        let armed = Gate {
            paused: false,
            ready_since: Some(t(5_000)),
        };
        st.apply(&ev("Stop", "a"), t(6_000));
        assert!(st.awake(armed));
        st.dismiss_all(); // "not now"
        assert!(!st.awake(armed));
        st.apply(&ev("UserPromptSubmit", "a"), t(7_000));
        st.apply(&ev("Stop", "a"), t(9_000)); // the next turn finishes: the cat wakes again
        assert!(st.awake(armed));
    }

    #[test]
    fn pause_beats_everything() {
        let mut st = SessionStore::default();
        st.apply(&tool("PermissionRequest", "a", "Bash"), t(0));
        let paused = Gate {
            paused: true,
            ready_since: None,
        };
        assert!(!st.awake(paused));
        assert!(st.waiting(paused).is_empty());
    }

    #[test]
    fn stale_pending_sessions_expire() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        st.apply(&ev("Stop", "b"), t(1_000_000));
        assert!(!st.expire(t(1_799_999), DurationMs(1_800_000)));
        assert!(st.expire(t(1_800_000), DurationMs(1_800_000)));
        assert!(!st.get("a").unwrap().pending);
        assert!(st.get("b").unwrap().pending);
    }

    #[test]
    fn quiet_sessions_are_forgotten() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "a"), t(0));
        st.apply(&ev("UserPromptSubmit", "a"), t(10));
        st.apply(&ev("Stop", "b"), t(0));
        st.expire(t(1_800_009), DurationMs(1_800_000));
        assert!(st.get("a").is_some(), "quiet for less than the stale time");
        assert!(!st.get("b").unwrap().pending);
        st.expire(t(1_800_010), DurationMs(1_800_000));
        assert!(st.get("a").is_none());
        // b's wait expired just now: it is forgotten one stale time later.
        assert!(st.get("b").is_some());
        st.expire(t(3_600_009), DurationMs(1_800_000));
        assert!(st.get("b").is_none());
    }

    #[test]
    fn the_store_is_capped_and_makes_room_from_quiet_sessions() {
        let mut st = SessionStore::default();
        for i in 0..MAX_SESSIONS {
            st.apply(&ev("Stop", &format!("s{i}")), t(0));
        }
        assert_eq!(st.apply(&ev("Stop", "new"), t(5_000)), Outcome::Unchanged);
        assert!(st.get("new").is_none());
        st.apply(&ev("UserPromptSubmit", "s7"), t(6_000));
        assert_eq!(
            st.apply(&ev("Stop", "new"), t(7_000)),
            attention("proj-new", WakeReason::Done)
        );
        assert!(st.get("s7").is_none());
        assert_eq!(st.sessions.len(), MAX_SESSIONS);
    }

    #[test]
    fn since_is_kept_while_already_pending_and_waiting_is_oldest_first() {
        let mut st = SessionStore::default();
        st.apply(&ev("Stop", "b"), t(5_000));
        st.apply(&ev("Stop", "a"), t(1_000));
        st.apply(&tool("PermissionRequest", "b", "Bash"), t(9_000)); // b changes reason, keeps since
        assert_eq!(st.get("b").unwrap().since, t(5_000));
        let names: Vec<_> = st
            .waiting(READY)
            .into_iter()
            .map(|v| v.project_name)
            .collect();
        assert_eq!(names, vec!["proj-a", "proj-b"]);
    }
}
