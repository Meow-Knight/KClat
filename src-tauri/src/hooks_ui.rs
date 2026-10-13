//! What the tray and the Settings window show and do about the Claude Code hooks (the file work itself is in
//! hooks_installer.rs).

use crate::hooks_installer::{self as hooks, Action, Report, Status};
use crate::log;
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
pub fn apply(action: Action) -> Result<String, String> {
    let Some((settings, exe)) = paths() else {
        return Err(
            "Could not locate %USERPROFILE%\\.claude\\settings.json or the hook program."
                .to_owned(),
        );
    };
    match hooks::apply(&settings, &exe, action) {
        Ok(report) => {
            log::info(format!(
                "hooks {action:?}: {report:?} (exe {})",
                exe.display()
            ));
            Ok(success_text(action, &report, &settings))
        }
        Err(e) => {
            log::warn(format!("hooks {action:?} failed: {e}"));
            Err(e.to_string())
        }
    }
}

fn success_text(action: Action, report: &Report, settings: &Path) -> String {
    let what = match action {
        Action::Install => "Klat hooks are installed",
        Action::Uninstall => "Klat hooks are removed",
    };
    match report {
        Report::NoChange => format!("{what}. Nothing needed to change in {}.", settings.display()),
        Report::Written { backup: Some(b) } => format!(
            "{what} in {}.\n\nYour previous settings were saved to {}.\n\nNew Claude Code sessions pick this up; restart open sessions if the cat stays asleep.",
            settings.display(),
            b.display()
        ),
        Report::Written { backup: None } => format!("{what} in {} (new file).", settings.display()),
    }
}
