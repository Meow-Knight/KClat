//! The Claude Code and Codex hooks: whether they are installed, installing and removing them, one agent at a time.

use crate::ipc::error::{ErrorCode, IpcError, Message};
use crate::services::hooks_service::{self, HooksError};
use klat_infra::hook_files::{agent_folder_exists, Action, Agent, Report, Status};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct HooksInfo {
    pub agent: Agent,
    pub status: Status,
    /// `~/.claude/settings.json` or `~/.codex/hooks.json`; None when the home folder is unknown.
    pub file_path: Option<String>,
    /// The agent's folder (`~/.claude`, `~/.codex`) exists, so it looks set up for this user. Install works
    /// either way.
    pub agent_found: bool,
}

/// Every agent, in the order the window lists them.
#[tauri::command]
pub fn hooks_status() -> Vec<HooksInfo> {
    Agent::ALL
        .into_iter()
        .map(|agent| {
            let file = hooks_service::paths(agent).map(|(f, _)| f);
            HooksInfo {
                agent,
                status: hooks_service::current_status(agent),
                file_path: file.as_ref().map(|f| f.display().to_string()),
                agent_found: file.as_deref().is_some_and(agent_folder_exists),
            }
        })
        .collect()
}

/// `action`: "install" (also repairs) or "uninstall", for one agent. File I/O runs off the main thread. Resolves to
/// what was done, rejects with why nothing was; the window words both.
#[tauri::command]
pub async fn hooks_apply(agent: Agent, action: Action) -> Result<Message, IpcError> {
    let result = tauri::async_runtime::spawn_blocking(move || hooks_service::apply(agent, action))
        .await
        .map_err(|e| {
            IpcError::new(
                ErrorCode::HooksIo,
                Message::new("hooks.error.io").with("detail", e.to_string()),
            )
        })?;
    match result {
        Ok((report, file)) => Ok(done(agent, action, &report, &file)),
        Err(e) => Err(e.into()),
    }
}

/// What Install or Uninstall did, to which file. Installing for Codex has its own texts: Codex runs a new hook
/// only after the user trusts it in `/hooks`.
fn done(agent: Agent, action: Action, report: &Report, settings: &Path) -> Message {
    let prefix = match (agent, action) {
        (Agent::ClaudeCode, Action::Install) => "hooks.install",
        (Agent::Codex, Action::Install) => "hooks.codex.install",
        (_, Action::Uninstall) => "hooks.uninstall",
    };
    let path = settings.display().to_string();
    match report {
        Report::NoChange => Message::new(&format!("{prefix}.noChange")).with("path", path),
        Report::Written { backup: Some(b) } => Message::new(&format!("{prefix}.written"))
            .with("path", path)
            .with("backup", b.display().to_string()),
        Report::Written { backup: None } => {
            Message::new(&format!("{prefix}.newFile")).with("path", path)
        }
    }
}

impl From<HooksError> for IpcError {
    fn from(e: HooksError) -> Self {
        match e {
            HooksError::NotFound => {
                IpcError::new(ErrorCode::HooksNotFound, Message::new("hooks.notFound"))
            }
            HooksError::Install(e, file) => {
                let mut ipc = IpcError::from(e);
                let name = file.file_name().unwrap_or(file.as_os_str());
                ipc.message = ipc.message.with("file", name.to_string_lossy());
                ipc
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klat_core::i18n::tf;
    use klat_core::settings::UiLanguage;
    use klat_infra::hook_files::InstallError;
    use serde_json::{json, Value};
    use std::io;
    use std::path::PathBuf;

    /// What Install/Uninstall said before the texts moved to the windows, and how they render now.
    const FIXTURE: &str = include_str!("../../../../tests/fixtures/hooksMessages.json");

    fn render(lang: UiLanguage, m: &Message) -> String {
        let vars: Vec<_> = m
            .vars
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        tf(lang, &m.key, &vars)
    }

    /// Every outcome, from the same inputs the fixture was captured with.
    fn outcomes() -> Vec<(String, Result<Message, IpcError>)> {
        let settings = Path::new("/home/me/.claude/settings.json");
        let backup = PathBuf::from("/home/me/.claude/settings.json.bak.1");
        let codex = Path::new("/home/me/.codex/hooks.json");
        let codex_backup = PathBuf::from("/home/me/.codex/hooks.json.bak.1");
        let mut out = Vec::new();
        for (agent, action, name) in [
            (Agent::ClaudeCode, Action::Install, "install"),
            (Agent::ClaudeCode, Action::Uninstall, "uninstall"),
            (Agent::Codex, Action::Install, "codex.install"),
            (Agent::Codex, Action::Uninstall, "codex.uninstall"),
        ] {
            let (file, backup) = match agent {
                Agent::ClaudeCode => (settings, backup.clone()),
                Agent::Codex => (codex, codex_backup.clone()),
            };
            let reports = [
                ("noChange", Report::NoChange),
                (
                    "written",
                    Report::Written {
                        backup: Some(backup.clone()),
                    },
                ),
                ("newFile", Report::Written { backup: None }),
            ];
            for (report_name, report) in reports {
                out.push((
                    format!("{name}.{report_name}"),
                    Ok(done(agent, action, &report, file)),
                ));
            }
        }
        let errors = [
            ("brokenJson", InstallError::BrokenJson("line 1".into())),
            (
                "unexpectedShape",
                InstallError::UnexpectedShape("\"hooks\""),
            ),
            (
                "exeMissing",
                InstallError::ExeMissing(PathBuf::from("/x/klat-hook")),
            ),
            ("changedConcurrently", InstallError::ChangedConcurrently),
            ("io", InstallError::Io(io::Error::other("disk full"))),
        ];
        for (name, e) in errors {
            out.push((
                format!("error.{name}"),
                Err(HooksError::Install(e, settings.to_path_buf()).into()),
            ));
        }
        out.push((
            "codex.error.brokenJson".into(),
            Err(HooksError::Install(
                InstallError::BrokenJson("line 1".into()),
                codex.to_path_buf(),
            )
            .into()),
        ));
        out.push(("notFound".into(), Err(HooksError::NotFound.into())));
        out
    }

    #[test]
    fn every_outcome_says_what_it_said_before() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        let outcomes = outcomes();
        assert_eq!(outcomes.len(), cases.len());
        for (name, outcome) in outcomes {
            let case = cases
                .iter()
                .find(|c| c["case"] == name.as_str())
                .unwrap_or_else(|| panic!("{name} is not in the fixture"));
            let (message, code) = match &outcome {
                Ok(m) => (m, None),
                Err(e) => (&e.message, Some(json!(e.code))),
            };
            assert_eq!(json!(message), case["message"], "{name}");
            assert_eq!(code, case.get("code").cloned(), "{name}");
            assert_eq!(case["ok"], json!(outcome.is_ok()), "{name}");
            assert_eq!(render(UiLanguage::En, message), case["en"], "{name}");
            assert_eq!(render(UiLanguage::Vi, message), case["vi"], "{name}");
        }
    }
}
