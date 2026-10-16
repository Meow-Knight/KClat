//! What the Settings window shows and does about the Claude Code hooks (the file work itself is in
//! klat_infra::claude_settings). Messages are in the Platform Language; the log stays in English.

// target: services::hooks_service

use klat_core::i18n::{t, tf};
use klat_core::settings::UiLanguage;
use klat_infra::claude_settings::{self as hooks, Action, InstallError, Report, Status};
use std::path::{Path, PathBuf};

/// `~/.claude/settings.json` and the hook adapter next to the app, if both can be located.
pub fn paths() -> Option<(PathBuf, PathBuf)> {
    Some((
        hooks::user_settings_path()?,
        hooks::default_exe_path().ok()?,
    ))
}

pub fn current_status() -> Status {
    paths().map_or(Status::NotInstalled, |(settings, exe)| {
        hooks::status_on_disk(&settings, &exe)
    })
}

/// Installs or removes the hooks. Ok and Err both carry the text to show the user. Does file I/O: call it off
/// the UI thread.
pub fn apply(lang: UiLanguage, action: Action) -> Result<String, String> {
    let Some((settings, exe)) = paths() else {
        return Err(t(lang, "hooks.notFound"));
    };
    match hooks::apply(&settings, &exe, action) {
        Ok(report) => {
            log::info!("hooks {action:?}: {report:?} (exe {})", exe.display());
            Ok(success_text(lang, action, &report, &settings))
        }
        Err(e) => {
            log::warn!("hooks {action:?} failed: {e}");
            Err(error_text(lang, &e))
        }
    }
}

fn success_text(lang: UiLanguage, action: Action, report: &Report, settings: &Path) -> String {
    let prefix = match action {
        Action::Install => "hooks.install",
        Action::Uninstall => "hooks.uninstall",
    };
    let path = settings.display().to_string();
    match report {
        Report::NoChange => tf(lang, &format!("{prefix}.noChange"), &[("path", &path)]),
        Report::Written { backup: Some(b) } => tf(
            lang,
            &format!("{prefix}.written"),
            &[("path", &path), ("backup", &b.display().to_string())],
        ),
        Report::Written { backup: None } => {
            tf(lang, &format!("{prefix}.newFile"), &[("path", &path)])
        }
    }
}

fn error_text(lang: UiLanguage, e: &InstallError) -> String {
    match e {
        InstallError::BrokenJson(detail) => {
            tf(lang, "hooks.error.brokenJson", &[("detail", detail)])
        }
        InstallError::UnexpectedShape(what) => {
            tf(lang, "hooks.error.unexpectedShape", &[("what", what)])
        }
        InstallError::ExeMissing(p) => tf(
            lang,
            "hooks.error.exeMissing",
            &[("path", &p.display().to_string())],
        ),
        InstallError::ChangedConcurrently => t(lang, "hooks.error.changedConcurrently"),
        InstallError::Io(err) => tf(lang, "hooks.error.io", &[("detail", &err.to_string())]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn every_outcome_has_a_text_in_both_languages() {
        let settings = Path::new("/home/me/.claude/settings.json");
        let reports = [
            Report::NoChange,
            Report::Written {
                backup: Some(PathBuf::from("/home/me/.claude/settings.json.bak.1")),
            },
            Report::Written { backup: None },
        ];
        for lang in [UiLanguage::En, UiLanguage::Vi] {
            for action in [Action::Install, Action::Uninstall] {
                for report in &reports {
                    let text = success_text(lang, action, report, settings);
                    assert!(text.contains("/home/me/.claude/settings.json"), "{text}");
                    assert!(!text.contains('{'), "unfilled placeholder: {text}");
                }
            }
            let errors = [
                InstallError::BrokenJson("line 1".into()),
                InstallError::UnexpectedShape("\"hooks\""),
                InstallError::ExeMissing(PathBuf::from("/x/klat-hook")),
                InstallError::ChangedConcurrently,
                InstallError::Io(io::Error::other("disk full")),
            ];
            for e in &errors {
                let text = error_text(lang, e);
                assert!(!text.contains('{') && !text.starts_with("hooks."), "{text}");
            }
        }
    }

    #[test]
    fn vietnamese_texts_are_vietnamese() {
        let text = success_text(
            UiLanguage::Vi,
            Action::Install,
            &Report::NoChange,
            Path::new("s.json"),
        );
        assert!(text.contains("đã được cài"), "{text}");
    }
}
