//! The Claude Code hooks: whether they are installed, installing and removing them.

use crate::ipc::error::{ErrorCode, IpcError, Message};
use crate::services::hooks_service::{self, HooksError};
use klat_infra::claude_settings::{Action, Report, Status};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct HooksInfo {
    pub status: Status,
    /// `~/.claude/settings.json`; None when the home folder is unknown.
    pub settings_path: Option<String>,
}

#[tauri::command]
pub fn hooks_status() -> HooksInfo {
    HooksInfo {
        status: hooks_service::current_status(),
        settings_path: hooks_service::paths().map(|(s, _)| s.display().to_string()),
    }
}

/// `action`: "install" (also repairs) or "uninstall". File I/O runs off the main thread. Resolves to what was
/// done, rejects with why nothing was; the window words both.
#[tauri::command]
pub async fn hooks_apply(action: Action) -> Result<Message, IpcError> {
    let result = tauri::async_runtime::spawn_blocking(move || hooks_service::apply(action))
        .await
        .map_err(|e| {
            IpcError::new(
                ErrorCode::HooksIo,
                Message::new("hooks.error.io").with("detail", e.to_string()),
            )
        })?;
    match result {
        Ok((report, settings)) => Ok(done(action, &report, &settings)),
        Err(e) => Err(e.into()),
    }
}

/// What Install or Uninstall did, to which file.
fn done(action: Action, report: &Report, settings: &Path) -> Message {
    let prefix = match action {
        Action::Install => "hooks.install",
        Action::Uninstall => "hooks.uninstall",
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
            HooksError::Install(e) => e.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use klat_core::i18n::tf;
    use klat_core::settings::UiLanguage;
    use klat_infra::claude_settings::InstallError;
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
        let mut out = Vec::new();
        for (action, name) in [
            (Action::Install, "install"),
            (Action::Uninstall, "uninstall"),
        ] {
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
                    Ok(done(action, &report, settings)),
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
            out.push((format!("error.{name}"), Err(HooksError::Install(e).into())));
        }
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
