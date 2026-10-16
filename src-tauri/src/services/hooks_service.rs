//! The Claude Code hooks from the Settings window: where the files are, whether the hooks are installed, installing
//! and removing them (the file work itself is klat_infra::claude_settings). What to tell the user is the window's
//! business: see ipc::commands::hooks.

use klat_infra::claude_settings::{self as hooks, Action, InstallError, Report, Status};
use std::path::PathBuf;

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

/// Why Install or Uninstall did nothing.
#[derive(Debug)]
pub enum HooksError {
    /// No home folder, or no hook program next to the app.
    NotFound,
    Install(InstallError),
}

/// Installs or removes the hooks; on success, what was done and to which file. Does file I/O: call it off the UI
/// thread.
pub fn apply(action: Action) -> Result<(Report, PathBuf), HooksError> {
    let Some((settings, exe)) = paths() else {
        return Err(HooksError::NotFound);
    };
    match hooks::apply(&settings, &exe, action) {
        Ok(report) => {
            log::info!("hooks {action:?}: {report:?} (exe {})", exe.display());
            Ok((report, settings))
        }
        Err(e) => {
            log::warn!("hooks {action:?} failed: {e}");
            Err(HooksError::Install(e))
        }
    }
}
