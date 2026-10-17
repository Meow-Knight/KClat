//! The event body, the hook names and the agents that run the hooks.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The coding agents whose hooks Klat installs. Both spell their hooks the same way and send the same fields
/// (`session_id`, `cwd`, `hook_event_name`, `tool_name`), so one adapter serves both; Codex entries tell it
/// which agent runs it (`klat-hook --agent codex Stop`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(rename = "HooksAgent"))]
pub enum Agent {
    ClaudeCode,
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::ClaudeCode, Agent::Codex];

    /// As the `--agent` flag, the event body and the Settings window spell it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Codex => "codex",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.as_str() == name)
    }
}

impl fmt::Display for Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Body of `POST /event` (spec §8.3). Only names and ids: the hook never sends tool input, commands, messages or
/// transcripts, and only the last folder of the working directory.
///
/// `hook_event_name` stays a string on the wire, so the app can tell an unknown hook (422) from a broken body
/// (400). Read it with [`HookEvent::hook`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEvent {
    pub event_id: String,
    pub hook_event_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_type: Option<String>,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// Unix ms, when the hook ran.
    pub occurred_at: u64,
    /// Which agent ran the hook ([`Agent::as_str`]). Absent for Claude Code, and from hooks older than Codex
    /// support. A string, like the hook name, so an agent this version does not know is still a valid body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

impl HookEvent {
    /// The hook this event is for; None for a name this version does not know.
    pub fn hook(&self) -> Option<HookName> {
        HookName::parse(&self.hook_event_name)
    }

    /// The agent that ran the hook: Claude Code when the body does not say; None for a name this version does
    /// not know.
    pub fn agent(&self) -> Option<Agent> {
        self.agent
            .as_deref()
            .map_or(Some(Agent::ClaudeCode), Agent::parse)
    }
}

/// The hooks the app understands. What each one means for the cat is the app's business (`classify` in the
/// session store); this is only the list, spelled as Claude Code and Codex spell them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookName {
    Stop,
    StopFailure,
    PermissionRequest,
    Notification,
    UserPromptSubmit,
    PostToolUse,
    PostToolUseFailure,
    PermissionDenied,
    SessionEnd,
    /// Codex only: the user stopped the turn.
    Interrupt,
    /// Accepted but not installed: installs from the Claude Cat days registered it. Its events are ignored
    /// quietly instead of being logged as rejections, and Install/Repair removes the entry.
    SubagentStop,
}

impl HookName {
    pub const ALL: [HookName; 11] = [
        HookName::Stop,
        HookName::StopFailure,
        HookName::PermissionRequest,
        HookName::Notification,
        HookName::UserPromptSubmit,
        HookName::PostToolUse,
        HookName::PostToolUseFailure,
        HookName::PermissionDenied,
        HookName::SessionEnd,
        HookName::Interrupt,
        HookName::SubagentStop,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            HookName::Stop => "Stop",
            HookName::StopFailure => "StopFailure",
            HookName::PermissionRequest => "PermissionRequest",
            HookName::Notification => "Notification",
            HookName::UserPromptSubmit => "UserPromptSubmit",
            HookName::PostToolUse => "PostToolUse",
            HookName::PostToolUseFailure => "PostToolUseFailure",
            HookName::PermissionDenied => "PermissionDenied",
            HookName::SessionEnd => "SessionEnd",
            HookName::Interrupt => "Interrupt",
            HookName::SubagentStop => "SubagentStop",
        }
    }

    /// Exact, case-sensitive, like Claude Code's own names.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|h| h.as_str() == name)
    }

    /// Whether Install puts an entry for it into that agent's hooks file. Codex has no `Notification`,
    /// `StopFailure`, `PostToolUseFailure` or `PermissionDenied`; Claude Code has no `Interrupt`.
    pub const fn is_installed(self, agent: Agent) -> bool {
        use HookName::*;
        match agent {
            Agent::ClaudeCode => !matches!(self, SubagentStop | Interrupt),
            Agent::Codex => matches!(
                self,
                Stop | PermissionRequest | UserPromptSubmit | PostToolUse | SessionEnd | Interrupt
            ),
        }
    }

    /// The hooks Install registers for that agent, in a stable order.
    pub fn installed(agent: Agent) -> impl Iterator<Item = HookName> {
        Self::ALL.into_iter().filter(move |h| h.is_installed(agent))
    }
}

impl fmt::Display for HookName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> HookEvent {
        HookEvent {
            event_id: "id-1".into(),
            hook_event_name: "PermissionRequest".into(),
            notification_type: Some("permission_prompt".into()),
            session_id: "abc".into(),
            project_name: Some("ClauCat".into()),
            tool_name: Some("Bash".into()),
            occurred_at: 42,
            agent: None,
        }
    }

    #[test]
    fn the_wire_format_is_exactly_this() {
        assert_eq!(
            serde_json::to_string(&full()).unwrap(),
            r#"{"eventId":"id-1","hookEventName":"PermissionRequest","notificationType":"permission_prompt","sessionId":"abc","projectName":"ClauCat","toolName":"Bash","occurredAt":42}"#
        );
        let bare = HookEvent {
            notification_type: None,
            project_name: None,
            tool_name: None,
            ..full()
        };
        // Absent fields are left out, as the hook has always done.
        assert_eq!(
            serde_json::to_string(&bare).unwrap(),
            r#"{"eventId":"id-1","hookEventName":"PermissionRequest","sessionId":"abc","occurredAt":42}"#
        );
    }

    #[test]
    fn reads_what_older_hooks_sent() {
        // Before klat-protocol the hook built its body with json!{}: keys sorted, absent fields left out.
        let old = r#"{"eventId":"id-1","hookEventName":"Stop","occurredAt":42,"projectName":"ClauCat","sessionId":"abc123"}"#;
        let ev: HookEvent = serde_json::from_str(old).unwrap();
        assert_eq!(ev.hook(), Some(HookName::Stop));
        assert_eq!(ev.session_id, "abc123");
        assert_eq!(ev.agent(), Some(Agent::ClaudeCode));
        assert_eq!(ev.project_name.as_deref(), Some("ClauCat"));
        assert_eq!((ev.tool_name, ev.notification_type), (None, None));
    }

    #[test]
    fn codex_events_say_so() {
        let ev = HookEvent {
            agent: Some("codex".into()),
            ..full()
        };
        let text = serde_json::to_string(&ev).unwrap();
        assert!(
            text.ends_with(r#""occurredAt":42,"agent":"codex"}"#),
            "{text}"
        );
        let back: HookEvent = serde_json::from_str(&text).unwrap();
        assert_eq!(back.agent(), Some(Agent::Codex));
    }

    #[test]
    fn an_unknown_agent_is_still_a_valid_body() {
        let newer = r#"{"eventId":"1","hookEventName":"Stop","sessionId":"s","occurredAt":1,"agent":"cursor"}"#;
        let ev: HookEvent = serde_json::from_str(newer).unwrap();
        assert_eq!(ev.agent(), None);
    }

    #[test]
    fn agents_are_spelled_alike_everywhere() {
        for a in Agent::ALL {
            assert_eq!(Agent::parse(a.as_str()), Some(a));
            assert_eq!(a.to_string(), a.as_str());
            assert_eq!(serde_json::to_value(a).unwrap(), a.as_str());
        }
        assert_eq!(Agent::parse("Codex"), None);
    }

    #[test]
    fn ignores_fields_a_newer_hook_may_add() {
        let newer = r#"{"eventId":"1","hookEventName":"Stop","sessionId":"s","occurredAt":1,"source":"cursor"}"#;
        assert!(serde_json::from_str::<HookEvent>(newer).is_ok());
    }

    #[test]
    fn round_trips() {
        let ev = full();
        let back: HookEvent = serde_json::from_str(&serde_json::to_string(&ev).unwrap()).unwrap();
        assert_eq!(back, ev);
    }

    #[test]
    fn hook_names_are_spelled_as_claude_code_spells_them() {
        let names: Vec<&str> = HookName::ALL.iter().map(|h| h.as_str()).collect();
        assert_eq!(
            names,
            [
                "Stop",
                "StopFailure",
                "PermissionRequest",
                "Notification",
                "UserPromptSubmit",
                "PostToolUse",
                "PostToolUseFailure",
                "PermissionDenied",
                "SessionEnd",
                "Interrupt",
                "SubagentStop",
            ]
        );
        for h in HookName::ALL {
            assert_eq!(HookName::parse(h.as_str()), Some(h));
            assert_eq!(h.to_string(), h.as_str());
        }
    }

    #[test]
    fn unknown_names_do_not_parse() {
        for name in ["PreToolUse", "stop", "Stop ", "", "rm -rf"] {
            assert_eq!(HookName::parse(name), None, "{name:?}");
        }
    }

    #[test]
    fn claude_code_gets_everything_but_subagent_stop_and_interrupt() {
        let installed: Vec<HookName> = HookName::installed(Agent::ClaudeCode).collect();
        assert_eq!(installed.len(), 9);
        assert!(!installed.contains(&HookName::SubagentStop));
        assert!(!installed.contains(&HookName::Interrupt));
    }

    #[test]
    fn codex_gets_the_hooks_it_has() {
        let names: Vec<&str> = HookName::installed(Agent::Codex)
            .map(HookName::as_str)
            .collect();
        assert_eq!(
            names,
            [
                "Stop",
                "PermissionRequest",
                "UserPromptSubmit",
                "PostToolUse",
                "SessionEnd",
                "Interrupt",
            ]
        );
    }
}
