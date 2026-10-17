//! The Claude Code and Codex hooks from the Settings window: where the files are, whether the hooks are installed,
//! installing and removing them (the file work itself is klat_infra::hook_files). Each agent is installed on its
//! own: the user picks one, the other or both. What to tell the user is the window's business: see
//! ipc::commands::hooks.

use klat_infra::hook_files::{self as hooks, Action, Agent, InstallError, Report, Status};
use std::path::PathBuf;

/// The agent's hooks file and the hook adapter next to the app, if both can be located.
pub fn paths(agent: Agent) -> Option<(PathBuf, PathBuf)> {
    Some((
        hooks::hooks_file_path(agent)?,
        hooks::default_exe_path().ok()?,
    ))
}

pub fn current_status(agent: Agent) -> Status {
    paths(agent).map_or(Status::NotInstalled, |(file, exe)| {
        hooks::status_on_disk(&file, agent, &exe)
    })
}

/// Why Install or Uninstall did nothing.
#[derive(Debug)]
pub enum HooksError {
    /// No home folder, or no hook program next to the app.
    NotFound,
    /// The file it was about.
    Install(InstallError, PathBuf),
}

/// Installs or removes one agent's hooks; on success, what was done and to which file. Does file I/O: call it off
/// the UI thread.
pub fn apply(agent: Agent, action: Action) -> Result<(Report, PathBuf), HooksError> {
    let Some((file, exe)) = paths(agent) else {
        return Err(HooksError::NotFound);
    };
    match hooks::apply(&file, agent, &exe, action) {
        Ok(report) => {
            log::info!(
                "hooks {agent} {action:?}: {report:?} (exe {})",
                exe.display()
            );
            Ok((report, file))
        }
        Err(e) => {
            log::warn!("hooks {agent} {action:?} failed: {e} ({})", file.display());
            Err(HooksError::Install(e, file))
        }
    }
}
